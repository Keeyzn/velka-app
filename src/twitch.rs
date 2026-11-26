//! Statut live des chaînes. Seul endroit qui parle à l'API GraphQL non officielle de Twitch :
//! en cas d'échec ou de changement de format, on retombe sur `Unknown` sans bloquer.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

const GQL_URL: &str = "https://gql.twitch.tv/gql";
const CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

#[derive(Debug, Clone, PartialEq)]
pub enum LiveStatus {
    Live {
        viewers: u64,
        game: String,
        title: String,
        /// Début du live (secondes depuis 1970), si connu.
        started_at: Option<u64>,
    },
    Offline,
    /// La chaîne n'existe pas.
    Missing,
    /// Erreur réseau ou réponse inattendue : on laisse l'utilisateur essayer.
    Unknown,
}

/// Construit la requête groupée (alias `u0`, `u1`, …). Les noms invalides sont ignorés.
pub fn build_query(logins: &[String]) -> String {
    let mut q = String::from("query{");
    for (i, l) in logins.iter().enumerate() {
        if crate::login::normalize(l).as_deref() == Some(l.as_str()) {
            q.push_str(&format!(
                "u{i}:user(login:\"{l}\"){{stream{{createdAt viewersCount game{{displayName}}}} broadcastSettings{{title}}}} "
            ));
        }
    }
    q.push('}');
    q
}

/// Retire les emojis, que les polices embarquées ne savent pas dessiner (carrés vides).
pub fn strip_emoji(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            let n = c as u32;
            !(n >= 0x1F000 || (0x2600..=0x27BF).contains(&n) || n == 0xFE0F || n == 0x200D)
        })
        .collect()
}

/// Convertit `AAAA-MM-JJThh:mm:ssZ` en secondes depuis 1970 (UTC).
pub fn parse_rfc3339(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    let is = |k: usize, c: u8| b.get(k) == Some(&c);
    if b.len() < 19
        || !is(4, b'-')
        || !is(7, b'-')
        || !is(10, b'T')
        || !is(13, b':')
        || !is(16, b':')
    {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hh, mm, ss) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Jours depuis 1970 (algorithme « days from civil »).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// Formate une durée : « 2 h 14 min » ou « 35 min ».
pub fn format_duration(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}

/// Interprète la réponse pour chaque chaîne demandée.
pub fn parse_response(logins: &[String], body: &Value) -> HashMap<String, LiveStatus> {
    let mut out = HashMap::new();
    let data = body.get("data");
    for (i, l) in logins.iter().enumerate() {
        let status = match data.and_then(|d| d.get(format!("u{i}"))) {
            None => LiveStatus::Unknown,
            Some(Value::Null) => LiveStatus::Missing,
            Some(user) => match user.get("stream") {
                None => LiveStatus::Unknown,
                Some(Value::Null) => LiveStatus::Offline,
                Some(s) => LiveStatus::Live {
                    viewers: s.get("viewersCount").and_then(Value::as_u64).unwrap_or(0),
                    game: s
                        .pointer("/game/displayName")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    title: strip_emoji(
                        user.pointer("/broadcastSettings/title")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    )
                    .trim()
                    .to_string(),
                    started_at: s
                        .get("createdAt")
                        .and_then(Value::as_str)
                        .and_then(parse_rfc3339),
                },
            },
        };
        out.insert(l.clone(), status);
    }
    out
}

/// Envoie une requête GraphQL (bloquant : à appeler depuis un thread). Connexion réutilisée (agent partagé) ;
/// pendant un recul (429, 5xx, réseau absent), rien ne part et l'appel échoue tout de suite.
fn post_query(query: &str) -> Result<Value, ureq::Error> {
    use crate::net::{report, report_response, Outcome, Service};
    if !crate::net::allowed(Service::Gql) {
        return Err(ureq::Error::StatusCode(429));
    }
    let agent = crate::net::agent_status(Duration::from_secs(5));
    let payload = serde_json::json!({ "query": query });
    let mut r = match agent
        .post(GQL_URL)
        .header("Client-ID", CLIENT_ID)
        .send_json(&payload)
    {
        Ok(r) => r,
        Err(e) => {
            report(Service::Gql, Outcome::Unreachable);
            return Err(e);
        }
    };
    report_response(Service::Gql, &r);
    let code = r.status().as_u16();
    if code >= 400 {
        return Err(ureq::Error::StatusCode(code));
    }
    r.body_mut().read_json::<Value>()
}

/// Interroge Twitch (bloquant : à appeler depuis un thread). Pendant un recul, rien n'est demandé et les statuts
/// déjà connus sont gardés (carte vide).
pub fn fetch_status(logins: &[String]) -> HashMap<String, LiveStatus> {
    let unknown = || {
        logins
            .iter()
            .map(|l| (l.clone(), LiveStatus::Unknown))
            .collect()
    };
    if logins.is_empty() || !crate::net::allowed(crate::net::Service::Gql) {
        return HashMap::new();
    }
    match post_query(&build_query(logins)) {
        Ok(body) => parse_response(logins, &body),
        Err(e) => {
            crate::logging::log(&format!("Statut live indisponible : {e}"));
            unknown()
        }
    }
}

/// Requête groupée des photos de profil (alias `u0`, `u1`, …). Les noms invalides sont ignorés.
pub fn build_avatar_query(logins: &[String]) -> String {
    let mut q = String::from("query{");
    for (i, l) in logins.iter().enumerate() {
        if crate::login::normalize(l).as_deref() == Some(l.as_str()) {
            q.push_str(&format!(
                "u{i}:user(login:\"{l}\"){{profileImageURL(width:70)}} "
            ));
        }
    }
    q.push('}');
    q
}

/// Adresses de photos par chaîne. Seules les adresses en `https://` sont acceptées.
pub fn parse_avatar_response(logins: &[String], body: &Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(data) = body.get("data") else {
        return out;
    };
    for (i, l) in logins.iter().enumerate() {
        let url = data
            .get(format!("u{i}"))
            .and_then(|u| u.get("profileImageURL"))
            .and_then(Value::as_str);
        if let Some(u) = url {
            if u.starts_with("https://") && !u.chars().any(char::is_whitespace) {
                out.insert(l.clone(), u.to_string());
            }
        }
    }
    out
}

/// Nombre maximal d'alias par requête GraphQL : Twitch répond « 400 » au-delà d'une dizaine.
pub const MAX_ALIASES: usize = 10;

/// Interroge Twitch pour les photos de profil, par paquets de `MAX_ALIASES` chaînes (bloquant : à appeler depuis un
/// thread). Un paquet qui échoue n'empêche pas les autres.
pub fn fetch_avatar_urls(logins: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for chunk in logins.chunks(MAX_ALIASES) {
        match post_query(&build_avatar_query(chunk)) {
            Ok(body) => out.extend(parse_avatar_response(chunk, &body)),
            Err(e) => crate::logging::log(&format!("Photos de profil indisponibles : {e}")),
        }
    }
    out
}

/// Chaîne proposée par l'autocomplétion de la recherche.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub login: String,
    /// Spectateurs si la chaîne est en direct.
    pub viewers: Option<u64>,
}

/// Fragment utilisable pour l'autocomplétion : 2 à 25 caractères `a-z 0-9 _` (minuscules).
pub fn suggest_fragment(input: &str) -> Option<String> {
    let f = input.trim().trim_start_matches(['@', '#']).to_lowercase();
    let ok =
        (2..=25).contains(&f.len()) && f.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some(f)
}

/// Requête d'autocomplétion (la même que celle de la recherche de twitch.tv).
pub fn build_suggest_query(fragment: &str) -> Option<String> {
    let f = suggest_fragment(fragment)?;
    Some(format!(
        "query{{searchSuggestions(queryFragment:\"{f}\",requestID:\"velka\"){{edges{{node{{content{{__typename ... on SearchSuggestionChannel{{user{{login stream{{viewersCount}}}}}}}}}}}}}}}}"
    ))
}

/// Chaînes proposées, dans l'ordre de Twitch (les autres types de résultats sont ignorés).
pub fn parse_suggestions(body: &Value) -> Vec<Suggestion> {
    let edges = body
        .pointer("/data/searchSuggestions/edges")
        .and_then(Value::as_array);
    let mut out: Vec<Suggestion> = Vec::new();
    for e in edges.into_iter().flatten() {
        let Some(user) = e.pointer("/node/content/user") else {
            continue;
        };
        let Some(login) = user.get("login").and_then(Value::as_str) else {
            continue;
        };
        if crate::login::normalize(login).as_deref() != Some(login)
            || out.iter().any(|s| s.login == login)
        {
            continue;
        }
        out.push(Suggestion {
            login: login.to_string(),
            viewers: user.pointer("/stream/viewersCount").and_then(Value::as_u64),
        });
    }
    out
}

/// Interroge Twitch pour l'autocomplétion (bloquant : à appeler depuis un thread).
pub fn fetch_suggestions(fragment: &str) -> Vec<Suggestion> {
    let Some(q) = build_suggest_query(fragment) else {
        return Vec::new();
    };
    match post_query(&q) {
        Ok(body) => parse_suggestions(&body),
        Err(e) => {
            crate::logging::log(&format!("Autocomplétion indisponible : {e}"));
            Vec::new()
        }
    }
}

/// Chaîne en direct proposée en recommandation.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamRec {
    pub login: String,
    pub viewers: u64,
    pub game: String,
}

/// Un nom de jeu est accepté dans une requête seulement sans guillemet, barre oblique inverse ni caractère de contrôle.
fn safe_game_name(g: &str) -> bool {
    !g.is_empty() && g.len() <= 100 && !g.chars().any(|c| c == '"' || c == '\\' || c.is_control())
}

/// Dernier jeu diffusé par une chaîne (même hors ligne).
pub fn build_game_query(login: &str) -> Option<String> {
    (crate::login::normalize(login).as_deref() == Some(login)).then(|| {
        format!("query{{user(login:\"{login}\"){{broadcastSettings{{game{{displayName}}}}}}}}")
    })
}

pub fn parse_game(body: &Value) -> Option<String> {
    body.pointer("/data/user/broadcastSettings/game/displayName")
        .and_then(Value::as_str)
        .filter(|g| !g.is_empty())
        .map(str::to_string)
}

/// Lives les plus regardés d'un jeu, ou de toute la plateforme si `game` est `None`.
pub fn build_streams_query(game: Option<&str>) -> Option<String> {
    match game {
        Some(g) if !safe_game_name(g) => None,
        Some(g) => Some(format!(
            "query{{game(name:\"{g}\"){{streams(first:12){{edges{{node{{viewersCount broadcaster{{login}}}}}}}}}}}}"
        )),
        None => Some(
            "query{streams(first:12){edges{node{viewersCount game{displayName} broadcaster{login}}}}}".to_string(),
        ),
    }
}

pub fn parse_streams_list(body: &Value, game: Option<&str>) -> Vec<StreamRec> {
    let path = if game.is_some() {
        "/data/game/streams/edges"
    } else {
        "/data/streams/edges"
    };
    let mut out: Vec<StreamRec> = Vec::new();
    for e in body
        .pointer(path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(node) = e.get("node") else {
            continue;
        };
        let Some(login) = node.pointer("/broadcaster/login").and_then(Value::as_str) else {
            continue;
        };
        if crate::login::normalize(login).as_deref() != Some(login)
            || out.iter().any(|r| r.login == login)
        {
            continue;
        }
        let g = node
            .pointer("/game/displayName")
            .and_then(Value::as_str)
            .or(game)
            .unwrap_or("")
            .to_string();
        out.push(StreamRec {
            login: login.to_string(),
            viewers: node
                .get("viewersCount")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            game: g,
        });
    }
    out
}

/// Identifiant de la chaîne, badges de la chaîne et badges globaux (pour le chat).
pub fn build_badges_query(login: &str) -> Option<String> {
    (crate::login::normalize(login).as_deref() == Some(login)).then(|| {
        format!(
            "query{{user(login:\"{login}\"){{id broadcastBadges{{setID version imageURL(size:DOUBLE)}}}} badges{{setID version imageURL(size:DOUBLE)}}}}"
        )
    })
}

/// `(identifiant, badges globaux, badges de la chaîne)` ; `None` si la chaîne est introuvable ou la requête échoue.
pub fn fetch_badges(login: &str) -> Option<(String, Value, Value)> {
    let body = post_query(&build_badges_query(login)?).ok()?;
    let id = body.pointer("/data/user/id")?.as_str()?.to_string();
    let global = body.pointer("/data/badges").cloned().unwrap_or(Value::Null);
    let channel = body
        .pointer("/data/user/broadcastBadges")
        .cloned()
        .unwrap_or(Value::Null);
    Some((id, global, channel))
}

pub fn fetch_channel_game(login: &str) -> Option<String> {
    parse_game(&post_query(&build_game_query(login)?).ok()?)
}

pub fn fetch_streams_list(game: Option<&str>) -> Vec<StreamRec> {
    let Some(q) = build_streams_query(game) else {
        return Vec::new();
    };
    match post_query(&q) {
        Ok(body) => parse_streams_list(&body, game),
        Err(e) => {
            crate::logging::log(&format!("Recommandations indisponibles : {e}"));
            Vec::new()
        }
    }
}

/// Ordre des recommandations : 1) chaînes suivies en direct qui jouent au même jeu, 2) autres lives
/// de ce jeu, 3) autres chaînes suivies en direct, 4) lives les plus regardés. Sans doublon ni `exclude`.
pub fn rank_recommendations(
    game: Option<&str>,
    followed_live: &[StreamRec],
    same_game: &[StreamRec],
    top: &[StreamRec],
    exclude: &str,
    limit: usize,
) -> Vec<StreamRec> {
    let same = |r: &StreamRec| game.is_some_and(|g| r.game.eq_ignore_ascii_case(g));
    let mut followed: Vec<&StreamRec> = followed_live.iter().collect();
    followed.sort_by_key(|r| std::cmp::Reverse(r.viewers));
    let mut ordered: Vec<&StreamRec> = followed.iter().copied().filter(|r| same(r)).collect();
    ordered.extend(same_game.iter());
    ordered.extend(followed.iter().copied().filter(|r| !same(r)));
    ordered.extend(top.iter());
    let mut out: Vec<StreamRec> = Vec::new();
    for r in ordered {
        if r.login != exclude && !out.iter().any(|o| o.login == r.login) {
            out.push(r.clone());
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dates_et_durees() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2023-11-14T22:13:20Z"), Some(1_700_000_000));
        assert_eq!(parse_rfc3339("n'importe quoi"), None);
        assert_eq!(format_duration(8040), "2 h 14 min");
        assert_eq!(format_duration(2100), "35 min");
    }

    #[test]
    fn emojis_retires() {
        assert_eq!(strip_emoji("\u{1F334}GTA\u{1F334}"), "GTA");
        assert_eq!(strip_emoji("Café ça va"), "Café ça va");
    }

    #[test]
    fn requete() {
        let q = build_query(&["xqc".into(), "Mauvais Nom".into(), "a_b".into()]);
        assert!(q.contains("u0:user(login:\"xqc\")"));
        assert!(!q.contains("u1:"));
        assert!(q.contains("u2:user(login:\"a_b\")"));
    }

    #[test]
    fn reponse() {
        let logins: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let body = json!({"data": {
            "u0": {"stream": {"createdAt": "2023-11-14T22:13:20Z", "viewersCount": 1234, "game": {"displayName": "Jeu"}}, "broadcastSettings": {"title": "Titre"}},
            "u1": {"stream": null, "broadcastSettings": {"title": "x"}},
            "u2": null
        }});
        let r = parse_response(&logins, &body);
        assert_eq!(
            r["a"],
            LiveStatus::Live {
                viewers: 1234,
                game: "Jeu".into(),
                title: "Titre".into(),
                started_at: Some(1_700_000_000),
            }
        );
        assert_eq!(r["b"], LiveStatus::Offline);
        assert_eq!(r["c"], LiveStatus::Missing);
        assert_eq!(r["d"], LiveStatus::Unknown);
        assert_eq!(
            parse_response(&logins, &json!({"errors": []}))["a"],
            LiveStatus::Unknown
        );
    }

    /// Plus de 10 chaînes (cache vide) : une requête par paquet de 10 au plus, sinon Twitch répond 400.
    #[test]
    fn photos_par_paquets_de_dix() {
        let logins: Vec<String> = (0..25).map(|i| format!("chaine{i}")).collect();
        let tailles: Vec<usize> = logins.chunks(MAX_ALIASES).map(|c| c.len()).collect();
        assert_eq!(tailles, vec![10, 10, 5]);
        for c in logins.chunks(MAX_ALIASES) {
            assert!(build_avatar_query(c).matches("profileImageURL").count() <= MAX_ALIASES);
        }
    }

    #[test]
    fn requete_photos() {
        let q = build_avatar_query(&["xqc".into(), "Mauvais Nom".into(), "a_b".into()]);
        assert!(q.contains("u0:user(login:\"xqc\"){profileImageURL(width:70)}"));
        assert!(!q.contains("u1:"));
        assert!(q.contains("u2:user(login:\"a_b\")"));
    }

    #[test]
    fn reponse_photos() {
        let logins: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let body = json!({"data": {
            "u0": {"profileImageURL": "https://static-cdn.jtvnw.net/jtv_user_pictures/a-70x70.png"},
            "u1": {"profileImageURL": "http://static-cdn.jtvnw.net/b.png"},
            "u2": null
        }});
        let r = parse_avatar_response(&logins, &body);
        assert_eq!(
            r["a"],
            "https://static-cdn.jtvnw.net/jtv_user_pictures/a-70x70.png"
        );
        assert!(!r.contains_key("b"));
        assert!(!r.contains_key("c"));
        assert!(!r.contains_key("d"));
        assert!(parse_avatar_response(&logins, &json!({"errors": []})).is_empty());
    }

    #[test]
    fn requete_autocompletion() {
        let q = build_suggest_query(" @XQ ").unwrap();
        assert!(q.contains("queryFragment:\"xq\""));
        assert!(build_suggest_query("x").is_none());
        assert!(build_suggest_query("a\"b").is_none());
    }

    #[test]
    fn reponse_autocompletion() {
        let body = json!({"data": {"searchSuggestions": {"edges": [
            {"node": {"content": {"__typename": "SearchSuggestionChannel", "user": {"login": "chaine_test", "stream": {"viewersCount": 41519}}}}},
            {"node": {"content": {"__typename": "SearchSuggestionChannel", "user": {"login": "xqc247", "stream": null}}}},
            {"node": {"content": {"__typename": "SearchSuggestionGame"}}},
            {"node": {"content": {"__typename": "SearchSuggestionChannel", "user": {"login": "chaine_test", "stream": null}}}}
        ]}}});
        let r = parse_suggestions(&body);
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            Suggestion {
                login: "chaine_test".into(),
                viewers: Some(41519)
            }
        );
        assert_eq!(r[1].viewers, None);
        assert!(parse_suggestions(&json!({"errors": []})).is_empty());
    }

    #[test]
    fn requetes_recommandations() {
        assert!(build_game_query("chaine_test")
            .unwrap()
            .contains("user(login:\"chaine_test\")"));
        assert!(build_game_query("Mauvais Nom").is_none());
        assert!(build_streams_query(Some("VALORANT"))
            .unwrap()
            .contains("game(name:\"VALORANT\")"));
        assert!(build_streams_query(Some("a\"b")).is_none());
        assert!(build_streams_query(None)
            .unwrap()
            .starts_with("query{streams(first:12)"));
    }

    #[test]
    fn analyse_des_recommandations() {
        let body = json!({"data": {"game": {"streams": {"edges": [
            {"node": {"viewersCount": 2561, "broadcaster": {"login": "florescent"}}},
            {"node": {"viewersCount": 5, "broadcaster": {"login": "Mauvais Nom"}}},
            {"node": {"viewersCount": 9, "broadcaster": {"login": "florescent"}}}
        ]}}}});
        let r = parse_streams_list(&body, Some("VALORANT"));
        assert_eq!(
            r,
            vec![StreamRec {
                login: "florescent".into(),
                viewers: 2561,
                game: "VALORANT".into()
            }]
        );
        assert_eq!(parse_game(&json!({"data": {"user": {"broadcastSettings": {"game": {"displayName": "Hades II"}}}}})).as_deref(), Some("Hades II"));
        assert!(parse_game(&json!({"data": {"user": null}})).is_none());
    }

    #[test]
    fn ordre_des_recommandations() {
        let rec = |l: &str, v: u64, g: &str| StreamRec {
            login: l.into(),
            viewers: v,
            game: g.into(),
        };
        let followed = vec![
            rec("suivi_gta", 500, "GTA"),
            rec("suivi_valo_petit", 10, "VALORANT"),
            rec("suivi_valo", 300, "VALORANT"),
        ];
        let same = vec![
            rec("autre_valo", 9000, "VALORANT"),
            rec("suivi_valo", 300, "VALORANT"),
        ];
        let top = vec![rec("star", 100_000, "GTA")];
        let r = rank_recommendations(Some("VALORANT"), &followed, &same, &top, "moi", 10);
        let names: Vec<&str> = r.iter().map(|x| x.login.as_str()).collect();
        assert_eq!(
            names,
            [
                "suivi_valo",
                "suivi_valo_petit",
                "autre_valo",
                "suivi_gta",
                "star"
            ]
        );
        let r = rank_recommendations(None, &followed, &[], &top, "suivi_gta", 2);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|x| x.login != "suivi_gta"));
    }
}
