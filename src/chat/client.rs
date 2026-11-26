//! Connexion IRC Twitch en WebSocket, dans un thread dédié, avec reconnexion automatique.
//! Lecture anonyme par défaut ; avec un jeton valide (droit `chat:edit`), on peut écrire.
use super::parse::{parse_line, to_chat_message, ChatMessage};
use serde_json::Value;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const URL: &str = "wss://irc-ws.chat.twitch.tv:443";
/// Longueur maximale d'un message Twitch.
pub const MAX_LEN: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnState {
    Connecting,
    Connected,
    /// Reconnexion prévue dans N secondes.
    Reconnecting(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// Lecture seule, sans compte.
    Anonymous,
    /// Connecté en tant que ce pseudo : l'envoi est possible.
    Authenticated(String),
    /// Jeton refusé ou sans droit d'écriture (message en français).
    Invalid(String),
}

pub enum ChatEvent {
    State(ConnState),
    Auth(AuthState),
    Message(ChatMessage),
    /// Mode « followers-only » de la chaîne (`ROOMSTATE`) : vrai quand il est actif (valeur 0 ou plus, -1 = désactivé).
    FollowersOnly(bool),
    /// Notre compte a-t-il un badge qui dispense du mode followers-only (`USERSTATE`) : modérateur, VIP, diffuseur, abonné, équipe Twitch.
    Exempt(bool),
}

/// Valeur de l'étiquette `followers-only` d'un `ROOMSTATE` : actif si 0 (tous les followers) ou plus (minutes).
pub fn followers_only_from_tag(v: &str) -> Option<bool> {
    v.trim().parse::<i64>().ok().map(|n| n >= 0)
}

/// Les badges d'un `USERSTATE` dispensent-ils du mode followers-only ?
pub fn exempt_from_badges(badges: &str) -> bool {
    badges.split(',').any(|b| {
        let set = b.split('/').next().unwrap_or("");
        // VIP : confirmé par l'aide Twitch ; modérateur et diffuseur : constaté. Abonnés (et fondateurs) : non confirmé par
        // la documentation, mais on les laisse écrire plutôt que de bloquer à tort ; Twitch refuse lui-même si besoin.
        matches!(
            set,
            "moderator"
                | "vip"
                | "broadcaster"
                | "subscriber"
                | "founder"
                | "staff"
                | "admin"
                | "global_mod"
        )
    })
}

pub struct ChatClient {
    stop: Arc<AtomicBool>,
    out: Sender<String>,
}

impl ChatClient {
    /// `token` vide = lecture anonyme. `notify` est appelé depuis le thread réseau.
    pub fn start(
        login: &str,
        token: &str,
        notify: impl Fn(ChatEvent) + Send + 'static,
    ) -> ChatClient {
        let stop = Arc::new(AtomicBool::new(false));
        let (out, out_rx) = channel();
        let (flag, login, token) = (stop.clone(), login.to_string(), clean_token(token));
        std::thread::spawn(move || run(&login, &token, &flag, &out_rx, &notify));
        ChatClient { stop, out }
    }

    /// Envoie un message (ignoré si le texte est vide ; sans effet en lecture anonyme).
    pub fn send(&self, text: &str) {
        if let Some(t) = sanitize_outgoing(text) {
            let _ = self.out.send(t);
        }
    }
}

impl Drop for ChatClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Retire les espaces et le préfixe `oauth:` éventuel.
pub fn clean_token(t: &str) -> String {
    let t = t.trim();
    t.strip_prefix("oauth:").unwrap_or(t).trim().to_string()
}

/// Nettoie un message sortant : une seule ligne, 500 caractères au plus, jamais vide.
pub fn sanitize_outgoing(text: &str) -> Option<String> {
    let one_line: String = text
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect();
    let t: String = one_line.trim().chars().take(MAX_LEN).collect();
    (!t.is_empty()).then_some(t)
}

/// Lit la réponse de validation : `(pseudo, a le droit chat:edit)`.
pub fn parse_validate(v: &Value) -> Option<(String, bool)> {
    let login = v.get("login")?.as_str()?.to_string();
    let can_chat = v
        .get("scopes")
        .and_then(Value::as_array)
        .is_some_and(|s| s.iter().any(|x| x.as_str() == Some("chat:edit")));
    Some((login, can_chat))
}

enum Resolved {
    Ok(String),
    Invalid(String),
    /// Réseau indisponible : on réessaiera à la prochaine connexion.
    Unreachable,
}

fn resolve(token: &str) -> Resolved {
    let agent = crate::net::agent_status(Duration::from_secs(8));
    let resp = agent
        .get(crate::auth::VALIDATE_URL)
        .header("Authorization", format!("OAuth {token}"))
        .call();
    match resp {
        Err(_) => Resolved::Unreachable,
        Ok(mut r) if r.status().is_success() => match r
            .body_mut()
            .read_json::<Value>()
            .ok()
            .as_ref()
            .and_then(parse_validate)
        {
            Some((login, true)) => Resolved::Ok(login),
            Some((_, false)) => Resolved::Invalid(
                crate::i18n::tr("Ce jeton n'a pas le droit d'écrire dans le chat (chat:edit).")
                    .into(),
            ),
            None => Resolved::Invalid(
                crate::i18n::tr("Réponse de Twitch inattendue pour ce jeton.").into(),
            ),
        },
        Ok(_) => Resolved::Invalid(
            crate::i18n::tr("Jeton refusé par Twitch (expiré ou invalide).").into(),
        ),
    }
}

fn sleep_unless_stopped(stop: &AtomicBool, secs: u64) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run(
    login: &str,
    token: &str,
    stop: &AtomicBool,
    out: &Receiver<String>,
    notify: &dyn Fn(ChatEvent),
) {
    let mut delay = 1u64;
    let mut first = true;
    // `None` : pas encore résolu ; `Some(None)` : lecture anonyme ; `Some(Some(pseudo))` : authentifié.
    let mut identity: Option<Option<String>> = if token.is_empty() { Some(None) } else { None };
    if token.is_empty() {
        notify(ChatEvent::Auth(AuthState::Anonymous));
    }
    while !stop.load(Ordering::Relaxed) {
        notify(ChatEvent::State(if first {
            ConnState::Connecting
        } else {
            ConnState::Reconnecting(0)
        }));
        first = false;
        if identity.is_none() {
            match resolve(token) {
                Resolved::Ok(nick) => {
                    notify(ChatEvent::Auth(AuthState::Authenticated(nick.clone())));
                    identity = Some(Some(nick));
                }
                Resolved::Invalid(msg) => {
                    notify(ChatEvent::Auth(AuthState::Invalid(msg)));
                    identity = Some(None);
                }
                Resolved::Unreachable => {} // lecture anonyme cette fois-ci, nouvel essai ensuite
            }
        }
        let nick = identity.clone().flatten();
        let started = Instant::now();
        match session(
            login,
            nick.as_deref().map(|n| (n, token)),
            stop,
            out,
            notify,
        ) {
            Ok(()) => {}
            Err(SessionError::AuthFailed) => {
                notify(ChatEvent::Auth(AuthState::Invalid(
                    crate::i18n::tr("Connexion refusée avec ce jeton.").into(),
                )));
                identity = Some(None);
            }
            Err(SessionError::Other(e)) => crate::logging::log(&format!("Chat : {e}")),
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            delay = 1; // la connexion était stable : on repart de zéro
        }
        notify(ChatEvent::State(ConnState::Reconnecting(delay)));
        sleep_unless_stopped(stop, delay);
        delay = (delay * 2).min(30);
    }
}

enum SessionError {
    AuthFailed,
    Other(String),
}

fn set_read_timeout(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) {
    let t = Some(Duration::from_millis(250));
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(t);
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_mut().set_read_timeout(t);
        }
        _ => {}
    }
}

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

fn send(ws: &mut Ws, s: String) -> Result<(), SessionError> {
    ws.send(Message::text(s))
        .map_err(|e| SessionError::Other(format!("envoi impossible : {e}")))
}

fn session(
    login: &str,
    auth: Option<(&str, &str)>,
    stop: &AtomicBool,
    out: &Receiver<String>,
    notify: &dyn Fn(ChatEvent),
) -> Result<(), SessionError> {
    let (mut ws, _) = tungstenite::connect(URL)
        .map_err(|e| SessionError::Other(format!("connexion impossible : {e}")))?;
    set_read_timeout(&mut ws);
    let nano = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    send(&mut ws, "CAP REQ :twitch.tv/tags twitch.tv/commands".into())?;
    // Le jeton n'est jamais journalisé.
    match auth {
        Some((nick, token)) => {
            send(&mut ws, format!("PASS oauth:{token}"))?;
            send(&mut ws, format!("NICK {nick}"))?;
        }
        None => {
            send(&mut ws, "PASS SCHMOOZE".into())?;
            send(&mut ws, format!("NICK justinfan{}", 10_000 + nano % 80_000))?;
        }
    }
    send(&mut ws, format!("JOIN #{login}"))?;
    let my_login = auth.map(|(n, _)| n);

    while !stop.load(Ordering::Relaxed) {
        while let Ok(text) = out.try_recv() {
            if auth.is_some() {
                send(&mut ws, format!("PRIVMSG #{login} :{text}"))?;
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                for line in t.lines() {
                    let Some(m) = parse_line(line) else { continue };
                    match m.command.as_str() {
                        "PING" => {
                            let arg = m.params.first().cloned().unwrap_or_default();
                            send(&mut ws, format!("PONG :{arg}"))?;
                        }
                        "001" => notify(ChatEvent::State(ConnState::Connected)),
                        "RECONNECT" => return Ok(()),
                        "NOTICE"
                            if m.params
                                .last()
                                .is_some_and(|p| p.contains("authentication failed")) =>
                        {
                            return Err(SessionError::AuthFailed);
                        }
                        "ROOMSTATE" => {
                            if let Some(on) = m
                                .tags
                                .get("followers-only")
                                .and_then(|v| followers_only_from_tag(v))
                            {
                                notify(ChatEvent::FollowersOnly(on));
                            }
                        }
                        "USERSTATE" => {
                            if let Some(b) = m.tags.get("badges") {
                                notify(ChatEvent::Exempt(exempt_from_badges(b)));
                            }
                        }
                        "PRIVMSG" => {
                            if let Some(c) = to_chat_message(&m, my_login) {
                                notify(ChatEvent::Message(c));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(SessionError::Other(format!("connexion perdue : {e}"))),
        }
    }
    let _ = ws.close(None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jeton_nettoye() {
        assert_eq!(clean_token("  oauth:abc123 "), "abc123");
        assert_eq!(clean_token("abc123"), "abc123");
        assert_eq!(clean_token("   "), "");
    }

    #[test]
    fn messages_sortants() {
        assert_eq!(
            sanitize_outgoing("  salut\nça va  ").as_deref(),
            Some("salut ça va")
        );
        assert_eq!(sanitize_outgoing("   \n "), None);
        assert_eq!(
            sanitize_outgoing(&"a".repeat(900)).map(|s| s.len()),
            Some(MAX_LEN)
        );
    }

    #[test]
    fn mode_followers_only() {
        assert_eq!(followers_only_from_tag("-1"), Some(false));
        assert_eq!(followers_only_from_tag("0"), Some(true));
        assert_eq!(followers_only_from_tag("30"), Some(true));
        assert_eq!(followers_only_from_tag("abc"), None);
        assert!(exempt_from_badges("moderator/1,subscriber/12"));
        assert!(exempt_from_badges("broadcaster/1"));
        assert!(exempt_from_badges("vip/1"));
        assert!(exempt_from_badges("subscriber/3,premium/1"));
        assert!(exempt_from_badges("founder/0"));
        assert!(!exempt_from_badges("premium/1,turbo/1"));
        assert!(!exempt_from_badges(""));
    }

    #[test]
    fn validation() {
        let ok = json!({"login": "moi", "scopes": ["chat:read", "chat:edit"]});
        assert_eq!(parse_validate(&ok), Some(("moi".into(), true)));
        let lecture = json!({"login": "moi", "scopes": ["chat:read"]});
        assert_eq!(parse_validate(&lecture), Some(("moi".into(), false)));
        assert_eq!(parse_validate(&json!({"status": 401})), None);
    }
}
