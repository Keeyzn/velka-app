//! Ligne de commande :
//! `velka <chaine> [--pip] [--no-chat] [--screen 2] [--volume 80] [--on-top] [--chat-left]
//!  [--stable] [--quality 720p] [--diag]`
use crate::config::Settings;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cli {
    pub channel: Option<String>,
    pub pip: bool,
    pub no_chat: bool,
    /// Numéro d'écran à partir de 1.
    pub screen: Option<usize>,
    pub volume: Option<u8>,
    pub on_top: bool,
    pub chat_left: bool,
    pub stable: bool,
    /// 0 = Source, 1 = 720p, 2 = 480p, 3 = Audio.
    pub quality: Option<u8>,
    /// Option interne `--wait-pid N` (relance après mise à jour) : attendre la fin du processus N avant de démarrer.
    pub wait_pid: Option<u32>,
    /// `--diag` : mode diagnostic, le lecteur écrit son journal détaillé `mpv.log`. Sans lui, aucun `mpv.log`.
    pub diag: bool,
    /// Arguments ignorés (inconnus ou invalides), pour le journal.
    pub warnings: Vec<String>,
}

pub fn parse_quality(v: &str) -> Option<u8> {
    match v.to_lowercase().as_str() {
        "source" | "best" => Some(0),
        "720p" | "720" => Some(1),
        "480p" | "480" => Some(2),
        "audio" => Some(3),
        "1080p" | "1080" => Some(4),
        _ => None,
    }
}

pub fn parse(args: &[String]) -> Cli {
    let mut cli = Cli::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        // Valeur de l'option : après `=` ou dans l'argument suivant.
        let value =
            |it: &mut std::slice::Iter<'_, String>| inline.clone().or_else(|| it.next().cloned());
        match flag {
            "--pip" => cli.pip = true,
            "--no-chat" => cli.no_chat = true,
            "--on-top" => cli.on_top = true,
            "--chat-left" => cli.chat_left = true,
            "--stable" => cli.stable = true,
            "--diag" => cli.diag = true,
            "--screen" => match value(&mut it)
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|n| *n >= 1)
            {
                Some(n) => cli.screen = Some(n),
                None => cli
                    .warnings
                    .push("--screen attend un numéro (1, 2…)".into()),
            },
            "--volume" => match value(&mut it).and_then(|v| v.parse::<u32>().ok()) {
                Some(n) => cli.volume = Some(n.min(130) as u8),
                None => cli
                    .warnings
                    .push("--volume attend un nombre de 0 à 130".into()),
            },
            "--quality" => match value(&mut it).as_deref().and_then(parse_quality) {
                Some(q) => cli.quality = Some(q),
                None => cli
                    .warnings
                    .push("--quality attend source, 720p, 480p ou audio".into()),
            },
            "--wait-pid" => match value(&mut it)
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| *n > 0)
            {
                Some(n) => cli.wait_pid = Some(n),
                None => cli
                    .warnings
                    .push("--wait-pid attend un numéro de processus".into()),
            },
            f if f.starts_with("--") => cli.warnings.push(format!("option inconnue : {f}")),
            _ => match crate::login::normalize(arg) {
                Some(l) if cli.channel.is_none() => cli.channel = Some(l),
                Some(_) => cli
                    .warnings
                    .push(format!("chaîne supplémentaire ignorée : {arg}")),
                None => cli.warnings.push(format!("nom de chaîne invalide : {arg}")),
            },
        }
    }
    cli
}

impl Cli {
    /// Applique les options aux réglages de cette session.
    pub fn apply(&self, s: &mut Settings) {
        if let Some(v) = self.volume {
            s.volume = v;
        }
        if let Some(q) = self.quality {
            s.quality = q;
        }
        if self.on_top {
            s.on_top = true;
        }
        if self.chat_left {
            s.chat_left = true;
        }
        if self.no_chat {
            s.chat_visible = false;
        }
        if self.stable {
            s.low_latency = false;
        }
    }
}

static CURRENT: OnceLock<Cli> = OnceLock::new();

/// Arguments du processus courant, analysés une seule fois.
pub fn current() -> &'static Cli {
    CURRENT.get_or_init(|| parse(&std::env::args().skip(1).collect::<Vec<_>>()))
}

// ---------- Transmission à l'instance déjà ouverte ----------

fn inbox_path() -> std::path::PathBuf {
    crate::logging::data_dir().join("open.txt")
}

/// Dépose les arguments pour l'instance en cours (elle les lit à son prochain rafraîchissement).
pub fn send_to_running(args: &[String]) {
    let _ = crate::config::write_atomic(
        &inbox_path(),
        args.join(
            "
",
        )
        .as_bytes(),
    );
}

/// Limites du canal entre instances. Le fichier est dans `%LOCALAPPDATA%`, réservé au compte Windows : seul
/// un programme de la même session peut y écrire. Une demande légitime fait quelques dizaines d'octets et est lue en
/// moins de 2 s.
const INBOX_MAX_BYTES: u64 = 4096;
const INBOX_MAX_ARGS: usize = 32;
const INBOX_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30);

/// Lit (et supprime) les arguments déposés par une seconde instance.
pub fn take_inbox() -> Option<Cli> {
    take_inbox_from(&inbox_path(), std::time::SystemTime::now())
}

/// Demande trop grosse ou périmée (laissée par une instance qui s'est fermée avant de la lire) : supprimée sans être
/// appliquée. Les options internes (`--wait-pid`, `--diag`) ne passent jamais par ce canal.
fn take_inbox_from(p: &std::path::Path, now: std::time::SystemTime) -> Option<Cli> {
    let meta = std::fs::metadata(p).ok()?;
    let fresh = meta
        .modified()
        .ok()
        .and_then(|m| now.duration_since(m).ok())
        .is_none_or(|age| age <= INBOX_MAX_AGE);
    let text = if meta.len() <= INBOX_MAX_BYTES && fresh {
        std::fs::read_to_string(p).ok()
    } else {
        None
    };
    let _ = std::fs::remove_file(p);
    let Some(text) = text else {
        crate::logging::log("Demande d'une seconde instance ignorée (trop grosse ou périmée)");
        return None;
    };
    let args: Vec<String> = text
        .lines()
        .take(INBOX_MAX_ARGS)
        .map(str::to_string)
        .collect();
    let mut cli = parse(&args);
    cli.wait_pid = None;
    cli.diag = false;
    Some(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Cli {
        parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn sans_argument() {
        assert_eq!(p(&[]), Cli::default());
    }

    #[test]
    fn chaine_et_options() {
        let c = p(&[
            "@XQC",
            "--pip",
            "--no-chat",
            "--screen",
            "2",
            "--volume=80",
            "--on-top",
            "--chat-left",
            "--stable",
            "--quality",
            "720p",
        ]);
        assert_eq!(c.channel.as_deref(), Some("xqc"));
        assert!(c.pip && c.no_chat && c.on_top && c.chat_left && c.stable);
        assert_eq!(
            (c.screen, c.volume, c.quality),
            (Some(2), Some(80), Some(1))
        );
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn url_et_options_avant_la_chaine() {
        let c = p(&["--volume", "200", "https://www.twitch.tv/xqc"]);
        assert_eq!(c.channel.as_deref(), Some("xqc"));
        assert_eq!(c.volume, Some(130));
    }

    #[test]
    fn erreurs_sans_plantage() {
        let c = p(&[
            "--volume",
            "abc",
            "--quality",
            "8k",
            "--screen",
            "0",
            "--bizarre",
            "a b",
        ]);
        assert_eq!(c.channel, None);
        assert_eq!(c.warnings.len(), 5);
    }

    /// Option interne de la relance : numéro de processus à attendre, sans avertissement, compatible avec une chaîne.
    #[test]
    fn attente_du_processus_precedent() {
        let c = p(&["--wait-pid", "4321"]);
        assert_eq!((c.wait_pid, c.channel.as_deref()), (Some(4321), None));
        assert!(c.warnings.is_empty());
        assert_eq!(p(&["xqc", "--wait-pid=7"]).wait_pid, Some(7));
        for faux in [
            &["--wait-pid", "abc"][..],
            &["--wait-pid", "0"],
            &["--wait-pid"],
        ] {
            let c = p(faux);
            assert_eq!((c.wait_pid, c.warnings.len()), (None, 1), "{faux:?}");
        }
    }

    /// `--diag` (journal du lecteur) : absent par défaut, sans avertissement, compatible avec une chaîne.
    #[test]
    fn mode_diagnostic() {
        assert!(!p(&["xqc"]).diag);
        let c = p(&["--diag", "xqc"]);
        assert_eq!((c.diag, c.channel.as_deref()), (true, Some("xqc")));
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn application_aux_reglages() {
        let mut s = Settings::default();
        p(&[
            "xqc",
            "--stable",
            "--no-chat",
            "--volume",
            "50",
            "--quality",
            "audio",
        ])
        .apply(&mut s);
        assert!(!s.low_latency && !s.chat_visible);
        assert_eq!((s.volume, s.quality), (50, 3));
    }

    /// canal entre instances borné (taille, âge) et sans options internes.
    #[test]
    fn boite_aux_lettres_bornee() {
        let dir = crate::testutil::TempDir::new("cli-inbox");
        let f = dir.path().join("open.txt");
        let now = std::time::SystemTime::now();
        std::fs::write(
            &f,
            "xqc
--volume
40
--wait-pid
1234
--diag",
        )
        .unwrap();
        let c = take_inbox_from(&f, now).unwrap();
        assert_eq!(
            (c.channel.as_deref(), c.volume, c.wait_pid, c.diag),
            (Some("xqc"), Some(40), None, false)
        );
        assert!(!f.exists(), "demande supprimée après lecture");
        assert!(take_inbox_from(&f, now).is_none(), "rien à lire");
        // Périmée : laissée plus de 30 s, supprimée sans être appliquée.
        std::fs::write(&f, "xqc").unwrap();
        assert!(take_inbox_from(&f, now + std::time::Duration::from_secs(60)).is_none());
        assert!(!f.exists());
        // Trop grosse.
        std::fs::write(&f, "a".repeat(5000)).unwrap();
        assert!(take_inbox_from(&f, std::time::SystemTime::now()).is_none());
        assert!(!f.exists());
    }
}
