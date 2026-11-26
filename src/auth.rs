//! Connexion à Twitch sans copier de jeton : « Device Code Grant Flow » (client public, sans secret).
//! L'application demande un code, ouvre la page d'autorisation de Twitch, puis attend l'accord de
//! l'utilisateur. Les jetons ne sont jamais écrits dans le journal.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Droits demandés : lire le chat, y écrire, et lire la liste des chaînes suivies.
pub const SCOPES: &str = "chat:read chat:edit user:read:follows";
/// Identifiant public de l'application Twitch de Velka (créée sur dev.twitch.tv, type « Public »).
/// Peut être remplacé à la compilation avec la variable d'environnement `VELKA_TWITCH_CLIENT_ID`.
const DEFAULT_CLIENT_ID: &str = "s1q70znw92yq4tu8xzkdqllf7nzgrz";

const DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
pub const VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";

pub fn client_id() -> &'static str {
    match option_env!("VELKA_TWITCH_CLIENT_ID") {
        Some(id) if !id.is_empty() => id,
        _ => DEFAULT_CLIENT_ID,
    }
}

pub fn available() -> bool {
    !client_id().is_empty()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    /// Adresse à ouvrir (contient déjà le code : l'utilisateur n'a plus qu'à cliquer sur « Autoriser »).
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Fin de validité (secondes depuis 1970).
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollResult {
    Pending,
    SlowDown,
    Done(Tokens),
    Failed(String),
}

/// Événements envoyés à l'interface pendant la connexion.
pub enum AuthEvent {
    Code(DeviceCode),
    Done { tokens: Tokens, login: String },
    Failed(String),
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Encode `clé=valeur&…` (application/x-www-form-urlencoded).
pub fn form_encode(pairs: &[(&str, &str)]) -> String {
    fn enc(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn parse_device(v: &Value) -> Option<DeviceCode> {
    Some(DeviceCode {
        device_code: v.get("device_code")?.as_str()?.to_string(),
        user_code: v.get("user_code")?.as_str()?.to_string(),
        verification_uri: v.get("verification_uri")?.as_str()?.to_string(),
        expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(1800),
        interval: v
            .get("interval")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .max(1),
    })
}

/// Interprète une réponse du point d'accès `/oauth2/token`.
pub fn parse_token_response(status: u16, v: &Value, now: u64) -> PollResult {
    if status == 200 {
        let Some(access) = v.get("access_token").and_then(Value::as_str) else {
            return PollResult::Failed("Réponse de Twitch inattendue.".into());
        };
        let refresh = v.get("refresh_token").and_then(Value::as_str).unwrap_or("");
        let expires_in = v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
        return PollResult::Done(Tokens {
            access_token: access.to_string(),
            refresh_token: refresh.to_string(),
            expires_at: now + expires_in,
        });
    }
    let msg = v
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    match msg.as_str() {
        "authorization_pending" => PollResult::Pending,
        "slow_down" => PollResult::SlowDown,
        m if m.contains("access_denied") || m.contains("denied") => {
            PollResult::Failed("Connexion refusée sur Twitch.".into())
        }
        m if m.contains("expired") => PollResult::Failed("Le code a expiré, recommencez.".into()),
        _ => PollResult::Failed("Twitch a refusé la connexion.".into()),
    }
}

/// À renouveler quand il reste moins de 10 minutes de validité (uniquement si on a un jeton de renouvellement).
pub fn needs_refresh(expires_at: u64, now: u64) -> bool {
    expires_at != 0 && expires_at < now + 600
}

fn agent() -> ureq::Agent {
    crate::net::agent_status(Duration::from_secs(8))
}

fn post_form(url: &str, pairs: &[(&str, &str)]) -> Result<(u16, Value), String> {
    let mut resp = agent()
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form_encode(pairs))
        .map_err(|_| "Impossible de joindre Twitch. Vérifiez votre connexion.".to_string())?;
    let status = resp.status().as_u16();
    let body: Value = resp.body_mut().read_json().unwrap_or(Value::Null);
    Ok((status, body))
}

pub fn request_device() -> Result<DeviceCode, String> {
    let (status, body) = post_form(
        DEVICE_URL,
        &[("client_id", client_id()), ("scopes", SCOPES)],
    )?;
    if status != 200 {
        return Err("Twitch a refusé la demande de connexion.".into());
    }
    parse_device(&body).ok_or_else(|| "Réponse de Twitch inattendue.".to_string())
}

pub fn poll_once(device_code: &str) -> Result<PollResult, String> {
    let (status, body) = post_form(
        TOKEN_URL,
        &[
            ("client_id", client_id()),
            ("scopes", SCOPES),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ],
    )?;
    Ok(parse_token_response(status, &body, now_secs()))
}

/// Échec d'un renouvellement : `Rejected` = Twitch refuse le jeton (il faut se reconnecter),
/// `Network` = coupure ou erreur passagère (les jetons sont gardés, on réessaie plus tard).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefreshError {
    Rejected,
    Network,
}

/// Interprète la réponse au renouvellement : seul un refus explicite de Twitch (400 ou 401) est définitif.
pub fn refresh_outcome(status: u16, body: &Value, now: u64) -> Result<Tokens, RefreshError> {
    match status {
        200 => match parse_token_response(status, body, now) {
            PollResult::Done(t) => Ok(t),
            _ => Err(RefreshError::Network),
        },
        400 | 401 => Err(RefreshError::Rejected),
        _ => Err(RefreshError::Network),
    }
}

/// Renouvelle un jeton arrivé à expiration (appel réseau : à lancer dans un fil de fond).
pub fn refresh(refresh_token: &str) -> Result<Tokens, RefreshError> {
    let (status, body) = post_form(
        TOKEN_URL,
        &[
            ("client_id", client_id()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ],
    )
    .map_err(|_| RefreshError::Network)?;
    refresh_outcome(status, &body, now_secs())
}

/// Le jeton n'est pas encore expiré (`expires_at` = 0 : jeton sans date, toujours considéré valable).
pub fn is_valid(expires_at: u64, now: u64) -> bool {
    expires_at == 0 || expires_at > now
}

/// Compte à qui appartient un jeton (réponse de `/oauth2/validate`).
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub login: String,
    pub user_id: String,
    pub scopes: Vec<String>,
}

pub fn parse_identity(v: &Value) -> Option<Identity> {
    let login = v.get("login")?.as_str()?.to_string();
    let user_id = v.get("user_id")?.as_str()?.to_string();
    if user_id.is_empty() || !user_id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let scopes = v
        .get("scopes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(Identity {
        login,
        user_id,
        scopes,
    })
}

pub fn fetch_identity(access_token: &str) -> Option<Identity> {
    use crate::net::{report, report_response, Outcome, Service};
    if !crate::net::allowed(Service::Id) {
        return None;
    }
    let mut resp = match agent()
        .get(VALIDATE_URL)
        .header("Authorization", format!("OAuth {access_token}"))
        .call()
    {
        Ok(r) => r,
        Err(_) => {
            report(Service::Id, Outcome::Unreachable);
            return None;
        }
    };
    report_response(Service::Id, &resp);
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.body_mut().read_json().ok()?;
    parse_identity(&v)
}

/// Durée pendant laquelle une validation réussie est réutilisée (Twitch demande une validation par heure).
const IDENTITY_TTL: Duration = Duration::from_secs(3600);

/// Empreinte du jeton, heure de la validation, identité.
type IdentityEntry = Option<([u8; 32], Instant, Identity)>;

fn identity_cache() -> std::sync::MutexGuard<'static, IdentityEntry> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<IdentityEntry>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Empreinte du jeton (le jeton lui-même n'est pas gardé dans le cache).
fn token_digest(token: &str) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(token.as_bytes()).into()
}

/// Identité du jeton, validée auprès de Twitch au plus une fois par heure. Une réponse 401 d'une autre
/// requête efface le cache (`forget_identity`).
pub fn cached_identity(access_token: &str) -> Option<Identity> {
    cached_identity_with(access_token, fetch_identity)
}

/// Comme `cached_identity`, avec la validation donnée (les tests en comptent les appels). Les appels simultanés
/// (validation horaire et actualisation des suivis au démarrage) attendent la même validation au lieu d'en lancer
/// deux : la mesure réelle du 22/11/2026 en montrait deux à la même seconde.
fn cached_identity_with(
    access_token: &str,
    fetch: impl Fn(&str) -> Option<Identity>,
) -> Option<Identity> {
    static FETCHING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let d = token_digest(access_token);
    let fresh = || {
        identity_cache()
            .as_ref()
            .filter(|(k, at, _)| *k == d && at.elapsed() < IDENTITY_TTL)
            .map(|(_, _, id)| id.clone())
    };
    if let Some(id) = fresh() {
        return Some(id);
    }
    let _one_at_a_time = FETCHING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(id) = fresh() {
        return Some(id);
    }
    let id = fetch(access_token)?;
    *identity_cache() = Some((d, Instant::now(), id.clone()));
    Some(id)
}

/// Oublie la validation mémorisée (jeton refusé, déconnexion).
pub fn forget_identity() {
    *identity_cache() = None;
}

/// `true` si la dernière validation date de plus d'une heure (ou n'a jamais eu lieu) pour ce jeton.
pub fn identity_due(access_token: &str) -> bool {
    let d = token_digest(access_token);
    identity_cache()
        .as_ref()
        .is_none_or(|(k, at, _)| *k != d || at.elapsed() >= IDENTITY_TTL)
}

/// Révoque le jeton auprès de Twitch (déconnexion). Bloquant : à appeler depuis un thread.
pub fn revoke(access_token: &str) {
    if access_token.is_empty() {
        return;
    }
    let _ = post_form(
        "https://id.twitch.tv/oauth2/revoke",
        &[("client_id", client_id()), ("token", access_token)],
    );
}

/// Pseudo du compte à qui appartient le jeton (même requête que `fetch_identity`).
pub fn fetch_login(access_token: &str) -> Option<String> {
    fetch_identity(access_token).map(|i| i.login)
}

/// Déroule toute la connexion dans le thread courant. `cancel` permet d'abandonner.
pub fn run_device_flow(cancel: &Arc<AtomicBool>, notify: &dyn Fn(AuthEvent)) {
    let device = match request_device() {
        Ok(d) => d,
        Err(e) => return notify(AuthEvent::Failed(e)),
    };
    let (device_code, expires, mut interval) = (
        device.device_code.clone(),
        device.expires_in,
        device.interval,
    );
    notify(AuthEvent::Code(device));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(expires) {
        // Attente découpée pour réagir vite à l'annulation.
        let wait_end = Instant::now() + Duration::from_secs(interval);
        while Instant::now() < wait_end {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        match poll_once(&device_code) {
            Ok(PollResult::Pending) => {}
            Ok(PollResult::SlowDown) => interval += 5,
            Ok(PollResult::Done(tokens)) => {
                let login = fetch_login(&tokens.access_token).unwrap_or_default();
                return notify(AuthEvent::Done { tokens, login });
            }
            Ok(PollResult::Failed(m)) => return notify(AuthEvent::Failed(m)),
            Err(_) => {} // coupure réseau passagère : on réessaie au prochain tour
        }
    }
    notify(AuthEvent::Failed("Le code a expiré, recommencez.".into()));
}

#[cfg(test)]
mod tests {
    /// Deux demandes simultanées pour le même jeton : une seule validation auprès de Twitch, puis le cache.
    #[test]
    fn une_seule_validation_pour_des_demandes_simultanees() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let calls = Arc::new(AtomicUsize::new(0));
        let token = "jeton-de-test-validation-simultanee";
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let calls = calls.clone();
                std::thread::spawn(move || {
                    super::cached_identity_with(token, |_| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        Some(super::Identity {
                            login: "essai".into(),
                            user_id: "1".into(),
                            scopes: vec![],
                        })
                    })
                })
            })
            .collect();
        for h in handles {
            assert_eq!(
                h.join().unwrap().map(|i| i.login),
                Some("essai".to_string())
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        super::forget_identity();
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn encodage_formulaire() {
        let s = form_encode(&[
            ("scopes", "chat:read chat:edit"),
            ("g", "urn:ietf:params:oauth:grant-type:device_code"),
        ]);
        assert_eq!(s, "scopes=chat%3Aread%20chat%3Aedit&g=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code");
    }

    #[test]
    fn reponse_appareil() {
        let v = json!({"device_code": "abc", "expires_in": 1800, "interval": 5, "user_code": "ABCDEFGH",
                       "verification_uri": "https://www.twitch.tv/activate?public=true&device-code=ABCDEFGH"});
        let d = parse_device(&v).unwrap();
        assert_eq!(
            (d.user_code.as_str(), d.interval, d.expires_in),
            ("ABCDEFGH", 5, 1800)
        );
        assert!(parse_device(&json!({})).is_none());
    }

    #[test]
    fn reponses_du_jeton() {
        let ok = json!({"access_token": "tok", "refresh_token": "ref", "expires_in": 14000});
        assert_eq!(
            parse_token_response(200, &ok, 1000),
            PollResult::Done(Tokens {
                access_token: "tok".into(),
                refresh_token: "ref".into(),
                expires_at: 15000
            })
        );
        assert_eq!(
            parse_token_response(400, &json!({"message": "authorization_pending"}), 0),
            PollResult::Pending
        );
        assert_eq!(
            parse_token_response(400, &json!({"message": "slow_down"}), 0),
            PollResult::SlowDown
        );
        assert!(matches!(
            parse_token_response(400, &json!({"message": "access_denied"}), 0),
            PollResult::Failed(_)
        ));
        assert!(matches!(
            parse_token_response(400, &json!({}), 0),
            PollResult::Failed(_)
        ));
        assert!(matches!(
            parse_token_response(200, &json!({}), 0),
            PollResult::Failed(_)
        ));
    }

    #[test]
    fn identite() {
        let v =
            json!({"login": "moi", "user_id": "123", "scopes": ["chat:read", "user:read:follows"]});
        let id = parse_identity(&v).unwrap();
        assert_eq!((id.login.as_str(), id.user_id.as_str()), ("moi", "123"));
        assert!(id.scopes.iter().any(|s| s == "user:read:follows"));
        assert!(parse_identity(&json!({"login": "moi", "user_id": "12/../3"})).is_none());
        assert!(parse_identity(&json!({})).is_none());
    }

    #[test]
    fn renouvellement_refus_ou_coupure() {
        let ok = json!({"access_token": "tok", "refresh_token": "ref", "expires_in": 100});
        assert!(matches!(refresh_outcome(200, &ok, 10), Ok(t) if t.expires_at == 110));
        // Refus explicite : définitif.
        assert_eq!(
            refresh_outcome(400, &json!({"message": "Invalid refresh token"}), 0),
            Err(RefreshError::Rejected)
        );
        assert_eq!(
            refresh_outcome(401, &Value::Null, 0),
            Err(RefreshError::Rejected)
        );
        // Erreur du serveur, limitation ou réponse illisible : passager, on garde les jetons.
        assert_eq!(
            refresh_outcome(500, &Value::Null, 0),
            Err(RefreshError::Network)
        );
        assert_eq!(
            refresh_outcome(429, &Value::Null, 0),
            Err(RefreshError::Network)
        );
        assert_eq!(
            refresh_outcome(200, &json!({}), 0),
            Err(RefreshError::Network)
        );
    }

    #[test]
    fn jeton_valable() {
        assert!(is_valid(0, 1000));
        assert!(is_valid(1001, 1000));
        assert!(!is_valid(1000, 1000));
    }

    #[test]
    fn renouvellement() {
        assert!(!needs_refresh(0, 1000)); // jeton collé à la main : pas de date
        assert!(needs_refresh(1500, 1000)); // moins de 10 minutes
        assert!(!needs_refresh(5000, 1000));
    }
}

#[cfg(test)]
mod reseau {
    use super::*;

    /// Test manuel : `cargo test auth::reseau -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn demande_de_code() {
        let d = request_device().expect("demande de code");
        println!(
            "code={} intervalle={} expire={} s url={}",
            d.user_code, d.interval, d.expires_in, d.verification_uri
        );
        assert!(!d.user_code.is_empty());
    }
}
