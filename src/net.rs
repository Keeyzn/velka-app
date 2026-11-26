//! Client HTTP commun : une seule façon de construire l'agent `ureq` (nom du programme, délai), pour que tous les
//! modules réseau se comportent pareil.
//!
//! - Agents **partagés** : un agent par réglage (délai, statuts, https seulement), réutilisé par tous les appels.
//!   Le pool de connexions de `ureq` garde les connexions ouvertes (keep-alive) : plus de nouvelle poignée TCP + TLS
//!   à chaque requête vers Twitch.
//! - **Recul** par service (`Gate`) : après un 429, un 5xx ou une erreur réseau, plus aucune requête vers ce service
//!   avant un délai qui double à chaque échec (15 s à 15 min), avec une part aléatoire pour que les clients ne
//!   reviennent pas tous en même temps ; `Retry-After` et `Ratelimit-Reset` sont respectés.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use ureq::config::ConfigBuilder;
use ureq::typestate::AgentScope;

/// Sonde de connexion Internet (bloquant : à appeler depuis un thread) : vrai si une connexion TCP s'ouvre vers
/// Twitch (port 443) en 2 s. Aucune donnée n'est envoyée. Sert à distinguer « pas d'Internet » d'une panne de Twitch.
pub fn internet_up() -> bool {
    use std::net::ToSocketAddrs;
    let Ok(addrs) = "gql.twitch.tv:443".to_socket_addrs() else {
        return false;
    };
    addrs
        .take(3)
        .any(|a| std::net::TcpStream::connect_timeout(&a, Duration::from_secs(2)).is_ok())
}

/// Durée pendant laquelle une connexion inactive reste dans le pool. `ureq` la garde 15 s par
/// défaut : entre deux salves d'actualisation (60 s au premier plan, 120 s en arrière-plan, `app/statuts.rs`) elle était
/// toujours refermée, donc nouvelle poignée TCP + TLS à chaque salve. Twitch garde une connexion inactive au moins
/// 150 s (mesuré le 24/11/2026 sur gql, api et id.twitch.tv : seconde requête servie sur la même connexion). `ureq`
/// vérifie qu'une connexion est encore ouverte avant de la reprendre.
pub const MAX_IDLE: Duration = Duration::from_secs(130);

/// Constructeur de base : nom « Velka », délai global et durée de vie des connexions inactives. À compléter pour un
/// besoin particulier.
pub fn builder(timeout: Duration) -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent("Velka")
        .max_idle_age(MAX_IDLE)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    /// 4xx/5xx = erreur.
    Errors,
    /// Statuts rendus tels quels.
    Status,
    /// `https://` seulement.
    Https,
}

/// Agent partagé pour ce réglage (créé à la première demande). Cloner un agent `ureq` partage son pool de connexions.
fn shared(timeout: Duration, kind: Kind) -> ureq::Agent {
    static AGENTS: OnceLock<Mutex<HashMap<(u128, Kind), ureq::Agent>>> = OnceLock::new();
    let map = AGENTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
    map.entry((timeout.as_millis(), kind))
        .or_insert_with(|| {
            let b = builder(timeout);
            match kind {
                Kind::Errors => b.build().into(),
                Kind::Status => b.http_status_as_error(false).build().into(),
                Kind::Https => b.https_only(true).build().into(),
            }
        })
        .clone()
}

/// Une réponse HTTP 4xx ou 5xx est une erreur (comportement habituel de `ureq`).
pub fn agent(timeout: Duration) -> ureq::Agent {
    shared(timeout, Kind::Errors)
}

/// Les statuts HTTP sont rendus tels quels (le code lit lui-même 400, 401…).
pub fn agent_status(timeout: Duration) -> ureq::Agent {
    shared(timeout, Kind::Status)
}

/// Refuse tout ce qui n'est pas en `https://` (images téléchargées depuis des adresses reçues de Twitch).
pub fn agent_https(timeout: Duration) -> ureq::Agent {
    shared(timeout, Kind::Https)
}

// ---------- Aléa léger (sans dépendance) ----------

/// Nombre pseudo-aléatoire dans [0, 1) : clés aléatoires de `RandomState` (graine du système) mêlées à l'heure.
pub fn random_unit() -> f64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    (h.finish() >> 11) as f64 / (1u64 << 53) as f64
}

/// Durée aléatoire entre 0 et `max`.
pub fn random_delay(max: Duration) -> Duration {
    max.mul_f64(random_unit())
}

// ---------- Recul par service ----------

/// Services interrogés en tâche de fond, chacun avec son propre recul.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Service {
    /// `gql.twitch.tv`
    Gql,
    /// `api.twitch.tv/helix`
    Helix,
    /// `id.twitch.tv` (validation du jeton)
    Id,
}

const BACKOFF_MIN: Duration = Duration::from_secs(15);
const BACKOFF_MAX: Duration = Duration::from_secs(15 * 60);
const UNREACHABLE_MAX: Duration = Duration::from_secs(120);

/// Délai avant de réessayer après `fails` échecs consécutifs (1 = premier échec). `hint` : délai demandé par le
/// serveur (`Retry-After`, `Ratelimit-Reset`). `unit` : aléa dans [0, 1) ; jusqu'à +50 % du délai.
pub fn backoff_delay(fails: u32, hint: Option<Duration>, unit: f64) -> Duration {
    let exp = BACKOFF_MIN
        .saturating_mul(1u32 << fails.saturating_sub(1).min(10))
        .min(BACKOFF_MAX);
    let base = hint.map_or(exp, |h| {
        h.max(exp.min(Duration::from_secs(60))).min(BACKOFF_MAX * 4)
    });
    base + base.mul_f64(0.5 * unit.clamp(0.0, 1.0))
}

#[derive(Default)]
struct Gate {
    fails: u32,
    until: Option<Instant>,
}

fn gates() -> std::sync::MutexGuard<'static, HashMap<Service, Gate>> {
    static GATES: OnceLock<Mutex<HashMap<Service, Gate>>> = OnceLock::new();
    GATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// `false` tant que ce service est en recul : la requête ne doit pas partir.
pub fn allowed(s: Service) -> bool {
    gates()
        .get(&s)
        .and_then(|g| g.until)
        .is_none_or(|t| Instant::now() >= t)
}

/// Résultat d'une requête, pour régler le recul du service.
pub enum Outcome {
    Ok,
    /// 429 ou 5xx ; avec le délai demandé par le serveur s'il y en a un.
    Failed(Option<Duration>),
    /// Serveur injoignable (pas de réseau) : même recul, plafonné à 2 min pour reprendre vite au retour du réseau.
    Unreachable,
    /// Requête réussie mais quota épuisé (`Ratelimit-Remaining: 0`) : attendre la remise à zéro.
    Exhausted(Duration),
}

pub fn report(s: Service, o: Outcome) {
    let mut g = gates();
    let gate = g.entry(s).or_default();
    match o {
        Outcome::Ok => *gate = Gate::default(),
        Outcome::Failed(_) | Outcome::Unreachable => {
            gate.fails = gate.fails.saturating_add(1);
            let d = match o {
                Outcome::Failed(hint) => backoff_delay(gate.fails, hint, random_unit()),
                _ => backoff_delay(gate.fails, None, random_unit()).min(UNREACHABLE_MAX),
            };
            gate.until = Some(Instant::now() + d);
            crate::logging::log(&format!(
                "Réseau : {s:?} en pause {} s (échec n° {})",
                d.as_secs(),
                gate.fails
            ));
        }
        Outcome::Exhausted(d) => {
            gate.fails = 0;
            gate.until = Some(Instant::now() + d + random_delay(Duration::from_secs(5)));
        }
    }
}

/// Délai demandé par le serveur : `Retry-After` (secondes) ou `Ratelimit-Reset` (heure Unix, Helix).
pub fn retry_hint<B>(resp: &ureq::http::Response<B>) -> Option<Duration> {
    let h = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
    };
    if let Some(s) = h("retry-after").and_then(|v| v.parse::<u64>().ok()) {
        return Some(Duration::from_secs(s));
    }
    let reset = h("ratelimit-reset").and_then(|v| v.parse::<u64>().ok())?;
    let now = u64::from(licfmt::now_secs());
    Some(Duration::from_secs(reset.saturating_sub(now).max(1)))
}

/// Classe une réponse reçue : 2xx (avec quota restant), 429/5xx (recul), autres (pas de recul : erreur du client).
pub fn report_response<B>(s: Service, resp: &ureq::http::Response<B>) {
    let code = resp.status().as_u16();
    if code == 429 || code >= 500 {
        return report(s, Outcome::Failed(retry_hint(resp)));
    }
    let remaining = resp
        .headers()
        .get("ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    match (remaining, retry_hint(resp)) {
        (Some(0), Some(d)) => report(s, Outcome::Exhausted(d)),
        _ => report(s, Outcome::Ok),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// la connexion survit à l'écart entre deux salves d'actualisation (120 s en arrière-plan),
    /// sans dépasser ce que Twitch garde (150 s mesurées).
    #[test]
    fn connexions_gardees_entre_deux_salves() {
        let c = builder(Duration::from_secs(3)).build();
        assert_eq!(c.max_idle_age(), MAX_IDLE);
        assert!(MAX_IDLE > Duration::from_secs(120) && MAX_IDLE < Duration::from_secs(150));
    }

    #[test]
    fn agents_construits() {
        let t = Duration::from_secs(3);
        let _ = (agent(t), agent_status(t), agent_https(t));
    }

    /// le même agent (donc le même pool de connexions) est rendu à chaque appel.
    #[test]
    fn agent_partage_entre_les_appels() {
        let t = Duration::from_millis(3210);
        let map = |a: &ureq::Agent| format!("{:p}", a.config());
        assert_eq!(map(&agent_status(t)), map(&agent_status(t)));
        assert_ne!(map(&agent_status(t)), map(&agent_https(t)));
    }

    /// délai qui double, plafonné, au moins le `Retry-After`, avec une part aléatoire bornée.
    #[test]
    fn recul_exponentiel_avec_alea() {
        assert_eq!(backoff_delay(1, None, 0.0), Duration::from_secs(15));
        assert_eq!(backoff_delay(2, None, 0.0), Duration::from_secs(30));
        assert_eq!(backoff_delay(3, None, 0.0), Duration::from_secs(60));
        assert_eq!(backoff_delay(40, None, 0.0), BACKOFF_MAX);
        assert_eq!(backoff_delay(1, None, 0.999_999).as_secs(), 22);
        assert_eq!(
            backoff_delay(1, Some(Duration::from_secs(120)), 0.0),
            Duration::from_secs(120)
        );
        assert_eq!(
            backoff_delay(1, Some(Duration::from_secs(1)), 0.0),
            Duration::from_secs(15),
            "jamais de boucle serrée"
        );
        assert_eq!(
            backoff_delay(1, Some(Duration::from_secs(1_000_000)), 0.0),
            BACKOFF_MAX * 4
        );
        for _ in 0..200 {
            let u = random_unit();
            assert!((0.0..1.0).contains(&u));
        }
        let (a, b) = (random_unit(), random_unit());
        assert_ne!(a, b, "deux tirages différents");
    }

    #[test]
    fn service_en_pause_apres_un_echec_puis_libere() {
        // Service réservé au test (aucun autre test ne l'utilise).
        let s = Service::Id;
        report(s, Outcome::Ok);
        assert!(allowed(s));
        report(s, Outcome::Failed(None));
        assert!(!allowed(s), "pause après un échec");
        report(s, Outcome::Ok);
        assert!(allowed(s));
        report(s, Outcome::Exhausted(Duration::from_secs(30)));
        assert!(!allowed(s), "quota épuisé : attendre la remise à zéro");
        report(s, Outcome::Unreachable);
        assert!(!allowed(s));
        report(s, Outcome::Ok);
    }

    #[test]
    fn indications_du_serveur() {
        let r = ureq::http::Response::builder()
            .status(429)
            .header("Retry-After", "42")
            .body(())
            .unwrap();
        assert_eq!(retry_hint(&r), Some(Duration::from_secs(42)));
        let reset = licfmt::now_secs() as u64 + 30;
        let r = ureq::http::Response::builder()
            .status(200)
            .header("Ratelimit-Reset", reset.to_string())
            .body(())
            .unwrap();
        let d = retry_hint(&r).unwrap().as_secs();
        assert!((29..=31).contains(&d));
        let r = ureq::http::Response::builder()
            .status(200)
            .body(())
            .unwrap();
        assert_eq!(retry_hint(&r), None);
    }

    /// Essai réseau, lancé à la main : `cargo test -p velka connexion_reutilisee -- --ignored --nocapture`.
    /// Deux requêtes vers gql.twitch.tv à 125 s d'écart avec l'agent partagé : la seconde doit passer par la même
    /// connexion TCP (même port local, lu par `netstat` pour ce processus).
    #[test]
    #[ignore]
    fn connexion_reutilisee_apres_125_s() {
        fn ports() -> Vec<String> {
            let out = std::process::Command::new("netstat")
                .args(["-ano", "-p", "TCP"])
                .output()
                .unwrap();
            let pid = std::process::id().to_string();
            String::from_utf8_lossy(&out.stdout)
                .lines()
                // Pas de filtre sur le nom de l'état : il est traduit (« ÉTABLIE » en français).
                .filter(|l| l.contains(":443") && l.split_whitespace().last() == Some(pid.as_str()))
                .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
                .collect()
        }
        let a = agent_status(Duration::from_secs(10));
        // Réponse lue jusqu'au bout, comme dans l'appli : sinon la connexion n'est pas rendue au pool.
        let _ = a
            .get("https://gql.twitch.tv/")
            .call()
            .unwrap()
            .body_mut()
            .read_to_string();
        let avant = ports();
        std::thread::sleep(Duration::from_secs(125));
        // Réponse lue jusqu'au bout, comme dans l'appli : sinon la connexion n'est pas rendue au pool.
        let _ = a
            .get("https://gql.twitch.tv/")
            .call()
            .unwrap()
            .body_mut()
            .read_to_string();
        let apres = ports();
        println!("avant : {avant:?} ; après : {apres:?}");
        assert_eq!(avant.len(), 1);
        assert_eq!(avant, apres, "même connexion réutilisée");
    }
}
