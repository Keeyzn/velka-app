//! Chaînes suivies par le compte Twitch connecté (API officielle Helix, droit `user:read:follows`).
//! Seule la liste des noms est récupérée ici : le statut en direct vient de `twitch.rs`.
//! Le jeton n'est jamais écrit dans le journal.
use crate::twitch::LiveStatus;
use serde_json::Value;
use std::time::Duration;

const FOLLOWED_URL: &str = "https://api.twitch.tv/helix/channels/followed";
/// 10 pages de 100 : au-delà de 1000 chaînes, la liste est tronquée.
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub enum FollowsError {
    /// Le jeton n'a pas le droit de lire les chaînes suivies : il faut se reconnecter.
    NeedsReconnect,
    Failed(String),
    /// Twitch a demandé de ralentir (429) ou ne répond pas : rien n'est demandé pendant le recul.
    Paused,
}

/// Noms de chaînes d'une page de résultats et curseur de la page suivante.
pub fn parse_followed(body: &Value) -> (Vec<String>, Option<String>) {
    let logins = body
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("broadcaster_login").and_then(Value::as_str))
        .filter(|l| crate::login::normalize(l).as_deref() == Some(*l))
        .map(str::to_string)
        .collect();
    let cursor = body
        .pointer("/pagination/cursor")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    (logins, cursor)
}

/// Adresse d'une page (valeurs encodées pour une adresse web).
pub fn page_url(user_id: &str, cursor: Option<&str>) -> String {
    let mut pairs = vec![("user_id", user_id), ("first", "100")];
    if let Some(c) = cursor {
        pairs.push(("after", c));
    }
    format!("{FOLLOWED_URL}?{}", crate::auth::form_encode(&pairs))
}

/// Streams en direct d'une page de `/streams/followed` et curseur de la page suivante.
pub fn parse_streams(body: &Value) -> (Vec<(String, LiveStatus)>, Option<String>) {
    let live = body
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|e| {
            e.get("type")
                .and_then(Value::as_str)
                .is_none_or(|t| t == "live")
        })
        .filter_map(|e| {
            let login = e.get("user_login").and_then(Value::as_str)?;
            if crate::login::normalize(login).as_deref() != Some(login) {
                return None;
            }
            let status = LiveStatus::Live {
                viewers: e.get("viewer_count").and_then(Value::as_u64).unwrap_or(0),
                game: e
                    .get("game_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                title: crate::twitch::strip_emoji(
                    e.get("title").and_then(Value::as_str).unwrap_or(""),
                )
                .trim()
                .to_string(),
                started_at: e
                    .get("started_at")
                    .and_then(Value::as_str)
                    .and_then(crate::twitch::parse_rfc3339),
            };
            Some((login.to_string(), status))
        })
        .collect();
    let cursor = body
        .pointer("/pagination/cursor")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    (live, cursor)
}

fn streams_url(user_id: &str, cursor: Option<&str>) -> String {
    page_url(user_id, cursor).replace("/channels/followed", "/streams/followed")
}

/// Résultat d'une actualisation : chaînes en direct, et la liste complète si elle a été demandée.
pub struct Snapshot {
    pub names: Option<Vec<String>>,
    pub live: Vec<(String, LiveStatus)>,
}

fn get_json(agent: &ureq::Agent, url: &str, access_token: &str) -> Result<Value, FollowsError> {
    use crate::net::{report, report_response, Outcome, Service};
    if !crate::net::allowed(Service::Helix) {
        return Err(FollowsError::Paused);
    }
    let mut resp = match agent
        .get(url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Client-Id", crate::auth::client_id())
        .call()
    {
        Ok(r) => r,
        Err(_) => {
            report(Service::Helix, Outcome::Unreachable);
            return Err(FollowsError::Failed("Impossible de joindre Twitch.".into()));
        }
    };
    report_response(Service::Helix, &resp);
    match resp.status().as_u16() {
        200 => {}
        429 => return Err(FollowsError::Paused),
        401 | 403 => {
            crate::auth::forget_identity();
            return Err(FollowsError::NeedsReconnect);
        }
        s => return Err(FollowsError::Failed(format!("Twitch a répondu {s}."))),
    }
    resp.body_mut()
        .read_json()
        .map_err(|_| FollowsError::Failed("Réponse de Twitch illisible.".into()))
}

/// Chaînes suivies en direct (une requête) et, si `with_names`, la liste complète
/// (bloquant : à appeler depuis un thread).
pub fn fetch_snapshot(access_token: &str, with_names: bool) -> Result<Snapshot, FollowsError> {
    if !crate::net::allowed(crate::net::Service::Helix) {
        return Err(FollowsError::Paused);
    }
    // Validation du jeton au plus une fois par heure (exigence Twitch), plus à chaque actualisation.
    let id = crate::auth::cached_identity(access_token)
        .ok_or_else(|| FollowsError::Failed("Compte Twitch injoignable.".into()))?;
    if !id.scopes.iter().any(|s| s == "user:read:follows") {
        return Err(FollowsError::NeedsReconnect);
    }
    let agent = crate::net::agent_status(Duration::from_secs(8));
    let mut live: Vec<(String, LiveStatus)> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let body = get_json(
            &agent,
            &streams_url(&id.user_id, cursor.as_deref()),
            access_token,
        )?;
        let (page, next) = parse_streams(&body);
        live.extend(page);
        match next {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    let names = if with_names {
        let mut all: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let body = get_json(
                &agent,
                &page_url(&id.user_id, cursor.as_deref()),
                access_token,
            )?;
            let (logins, next) = parse_followed(&body);
            all.extend(logins);
            match next {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Some(all)
    } else {
        None
    };
    Ok(Snapshot { names, live })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn analyse_d_une_page() {
        let body = json!({
            "data": [
                {"broadcaster_id": "1", "broadcaster_login": "chaine_test", "broadcaster_name": "Chaine_Test"},
                {"broadcaster_id": "2", "broadcaster_login": "Nom Invalide"},
                {"broadcaster_id": "3"}
            ],
            "pagination": {"cursor": "abc=="}
        });
        let (logins, cursor) = parse_followed(&body);
        assert_eq!(logins, vec!["chaine_test".to_string()]);
        assert_eq!(cursor.as_deref(), Some("abc=="));
        let (none, no_cursor) = parse_followed(&json!({"data": [], "pagination": {}}));
        assert!(none.is_empty() && no_cursor.is_none());
    }

    #[test]
    fn analyse_des_streams() {
        let body = json!({"data": [
            {"user_login": "chaine_test", "viewer_count": 40500, "game_name": "GTA", "title": "Titre", "started_at": "2023-11-14T22:13:20Z", "type": "live"},
            {"user_login": "Mauvais Nom", "viewer_count": 1, "type": "live"},
            {"user_login": "rerun", "viewer_count": 1, "type": ""}
        ], "pagination": {}});
        let (live, cursor) = parse_streams(&body);
        assert!(cursor.is_none());
        assert_eq!(live.len(), 1);
        assert_eq!(
            live[0].1,
            LiveStatus::Live {
                viewers: 40500,
                game: "GTA".into(),
                title: "Titre".into(),
                started_at: Some(1_700_000_000)
            }
        );
        assert!(streams_url("42", None)
            .starts_with("https://api.twitch.tv/helix/streams/followed?user_id=42"));
    }

    #[test]
    fn adresse_de_page() {
        assert_eq!(
            page_url("42", None),
            "https://api.twitch.tv/helix/channels/followed?user_id=42&first=100"
        );
        assert!(page_url("42", Some("a b==")).ends_with("&after=a%20b%3D%3D"));
    }
}
