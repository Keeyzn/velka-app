//! Lancement et suivi de Streamlink en processus de fond invisible.
//! Streamlink expose le flux sur un serveur HTTP local (127.0.0.1), lu ensuite par libmpv.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau : aucune indexation qui panique
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Le serveur local répond : on peut ouvrir `http://127.0.0.1:<port>/`.
    Ready(u16),
    /// Streamlink signale une coupure du flux (`true`) ou sa reprise (`false`).
    Break(bool),
    /// Streamlink s'est arrêté ou n'a pas démarré à temps.
    Failed(StreamError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamError {
    Offline,
    Network,
    /// Aucune connexion Internet (sonde `net::internet_up` négative après l'échec du flux).
    NoInternet,
    Timeout,
    Other(String),
}

impl StreamError {
    pub fn message(&self) -> String {
        match self {
            StreamError::Offline => crate::i18n::tr("La chaîne est hors ligne.").into(),
            StreamError::Network => {
                crate::i18n::tr("Pas de connexion à Twitch. Vérifiez votre réseau.").into()
            }
            StreamError::NoInternet => crate::i18n::tr("Pas de connexion Internet.").into(),
            StreamError::Timeout => crate::i18n::tr("Le flux met trop de temps à démarrer.").into(),
            StreamError::Other(m) => {
                format!("{} {m}", crate::i18n::tr("Le flux s'est interrompu :"))
            }
        }
    }
}

/// Spécification de qualité Streamlink pour l'indice des réglages (0 à 3).
pub fn quality_spec(q: u8) -> &'static str {
    match q {
        1 => "720p60,720p,best",
        2 => "480p,best",
        3 => "audio_only,audio",
        4 => "1080p60,1080p,best",
        _ => "best",
    }
}

/// Arguments fixes de Streamlink (le port local est ajouté juste après `--player-external-http-port`).
pub const STREAMLINK_ARGS: &[&str] = &[
    "--loglevel=info",
    "--stream-segment-threads=2",
    "--player-external-http",
    "--player-external-http-interface=127.0.0.1",
    "--player-external-http-port",
];

/// Arguments de Streamlink pour une chaîne.
pub fn build_args(login: &str, quality: u8, low_latency: bool, port: u16) -> Vec<String> {
    let mut a: Vec<String> = STREAMLINK_ARGS.iter().map(|s| s.to_string()).collect();
    a.push(port.to_string());
    if low_latency {
        a.push("--twitch-low-latency".into());
    }
    a.push(format!("twitch.tv/{login}"));
    a.push(quality_spec(quality).into());
    a
}

/// Choisit un port libre en ouvrant puis fermant un socket local.
pub fn free_port() -> Option<u16> {
    let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    l.local_addr().ok().map(|a| a.port())
}

/// Interprète une ligne de sortie de Streamlink.
pub fn classify_line(line: &str) -> Option<StreamEvent> {
    let l = line.to_lowercase();
    if l.contains("resuming stream output") {
        Some(StreamEvent::Break(false))
    } else if l.contains("advertisement") || l.contains("filtering out segments") {
        Some(StreamEvent::Break(true))
    } else {
        None
    }
}

/// Déduit l'erreur à afficher depuis les dernières lignes de sortie.
pub fn classify_failure(lines: &[String]) -> StreamError {
    let all = lines.join("\n").to_lowercase();
    if all.contains("no playable streams") || all.contains("offline") {
        StreamError::Offline
    } else if all.contains("unable to open url")
        || all.contains("max retries")
        || all.contains("connection")
        || all.contains("name or service")
        || all.contains("getaddrinfo")
    {
        StreamError::Network
    } else {
        let last = lines.iter().rev().find(|l| !l.trim().is_empty());
        StreamError::Other(
            last.map(|l| l.trim().to_string())
                .unwrap_or_else(|| crate::i18n::tr("arrêt inattendu").into()),
        )
    }
}

pub struct Stream {
    child: Child,
    stopped: Arc<AtomicBool>,
}

impl Stream {
    /// Démarre Streamlink ; les événements sont remis à `notify` depuis des threads de fond.
    pub fn start(
        exe: &Path,
        args: &[String],
        port: u16,
        notify: impl Fn(StreamEvent) + Send + Sync + 'static,
    ) -> Result<Stream, String> {
        let mut cmd = Command::new(exe);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(format!(
                    "{} {e}",
                    crate::i18n::tr("Impossible de lancer Streamlink :")
                ));
            }
        };
        crate::procs::attach(&child);

        let notify = Arc::new(notify);
        let stopped = Arc::new(AtomicBool::new(false));

        if let Some(stderr) = child.stderr.take() {
            let (n, st) = (notify.clone(), stopped.clone());
            std::thread::spawn(move || {
                let mut tail: Vec<String> = Vec::new();
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    crate::logging::log(&format!("streamlink: {line}"));
                    if let Some(ev) = classify_line(&line) {
                        n(ev);
                    }
                    tail.push(line);
                    if tail.len() > 8 {
                        tail.remove(0);
                    }
                }
                if !st.load(Ordering::Relaxed) {
                    let mut err = classify_failure(&tail);
                    // Une chaîne hors ligne reste hors ligne ; toute autre panne sans Internet est une coupure de connexion.
                    if err != StreamError::Offline && !crate::net::internet_up() {
                        err = StreamError::NoInternet;
                    }
                    n(StreamEvent::Failed(err));
                }
            });
        }

        let (n, st) = (notify, stopped.clone());
        std::thread::spawn(move || {
            let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(15) {
                if st.load(Ordering::Relaxed) {
                    return;
                }
                if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok() {
                    n(StreamEvent::Ready(port));
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            if !st.load(Ordering::Relaxed) {
                let err = if crate::net::internet_up() {
                    StreamError::Timeout
                } else {
                    StreamError::NoInternet
                };
                n(StreamEvent::Failed(err));
            }
        });

        Ok(Stream { child, stopped })
    }

    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments() {
        let a = build_args("xqc", 1, true, 5000);
        assert_eq!(
            a[..STREAMLINK_ARGS.len()],
            STREAMLINK_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()[..]
        );
        assert_eq!(
            a[STREAMLINK_ARGS.len()],
            "5000",
            "le port suit --player-external-http-port"
        );
        assert!(a.contains(&"--twitch-low-latency".to_string()));
        assert!(!a
            .iter()
            .any(|x| x.contains("Authorization") || x == "--config"));
        assert_eq!(a[a.len() - 2], "twitch.tv/xqc");
        assert_eq!(a[a.len() - 1], "720p60,720p,best");
        let b = build_args("xqc", 0, false, 5000);
        assert!(!b.contains(&"--twitch-low-latency".to_string()));
    }

    #[test]
    fn lignes_et_erreurs() {
        assert_eq!(
            classify_line("[stream.hls][info] Filtering out segments and pausing stream output"),
            Some(StreamEvent::Break(true))
        );
        assert_eq!(
            classify_line("[stream.hls][info] Resuming stream output"),
            Some(StreamEvent::Break(false))
        );
        assert_eq!(classify_line("autre chose"), None);
        assert_eq!(
            classify_failure(&["error: No playable streams found on this URL".into()]),
            StreamError::Offline
        );
        assert_eq!(
            classify_failure(&["error: Unable to open URL: Max retries exceeded".into()]),
            StreamError::Network
        );
    }

    #[test]
    fn port_libre() {
        assert!(free_port().is_some());
    }
}

#[cfg(test)]
mod reseau {
    use super::*;
    use crate::tools::{detect, Tool};
    use crate::twitch::{fetch_status, LiveStatus};

    /// Test manuel : `cargo test reseau -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn statut_et_streamlink() {
        let logins: Vec<String> = ["xqc", "zzzz_inexistante_999"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let st = fetch_status(&logins);
        println!("{st:?}");
        let live = logins
            .iter()
            .find(|l| matches!(st[*l], LiveStatus::Live { .. }));
        let exe = detect(Tool::Streamlink, "").expect("streamlink absent");
        let target = live.cloned().unwrap_or_else(|| "xqc".into());
        let port = free_port().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let mut s = Stream::start(&exe, &build_args(&target, 0, true, port), port, move |ev| {
            let _ = tx.lock().unwrap().send(ev);
        })
        .unwrap();
        let ev = rx.recv_timeout(Duration::from_secs(25));
        println!("{target}: {ev:?}");
        s.stop();
    }
}
