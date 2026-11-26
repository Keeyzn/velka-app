//! Emotes des extensions (7TV, BTTV, FFZ) et badges du chat : chargés une fois par live, en arrière-plan.
//! Les emotes externes sont reconnues par leur nom, mot par mot, dans le texte des messages.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use super::parse::Segment;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Données chargées pour une chaîne : nom d'emote → clé d'image, et badge (`set/version`) → clé d'image.
#[derive(Default, Clone)]
pub struct ChatExtras {
    pub emotes: HashMap<String, String>,
    pub badges: HashMap<String, String>,
}

fn agent() -> ureq::Agent {
    crate::net::agent(Duration::from_secs(8))
}

fn get_json(agent: &ureq::Agent, url: &str) -> Option<Value> {
    agent
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .read_json::<Value>()
        .ok()
}

/// Identifiant d'image sans caractère dangereux (pas de chemin ni d'URL).
fn clean(id: &str) -> Option<String> {
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| id.to_string())
}

/// BTTV : `[{id, code, animated}]` → `code` → `bttv_ID` (ou `bttva_ID` si animée).
pub fn parse_bttv(list: &Value, out: &mut HashMap<String, String>) {
    for e in list.as_array().into_iter().flatten() {
        let (Some(code), Some(id)) = (e["code"].as_str(), e["id"].as_str().and_then(clean)) else {
            continue;
        };
        let prefix = if e["animated"].as_bool() == Some(true) {
            "bttva"
        } else {
            "bttv"
        };
        out.insert(code.to_string(), format!("{prefix}_{id}"));
    }
}

/// FFZ (via l'API de BTTV) : `[{id (nombre), code}]` → `ffz_ID`.
pub fn parse_ffz(list: &Value, out: &mut HashMap<String, String>) {
    for e in list.as_array().into_iter().flatten() {
        let (Some(code), Some(id)) = (e["code"].as_str(), e["id"].as_u64()) else {
            continue;
        };
        out.insert(code.to_string(), format!("ffz_{id}"));
    }
}

/// 7TV : `[{name, data: {id, animated}}]` → `7tv_ID` (ou `7tva_ID` si animée).
pub fn parse_7tv(list: &Value, out: &mut HashMap<String, String>) {
    for e in list.as_array().into_iter().flatten() {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        let id = e
            .pointer("/data/id")
            .and_then(Value::as_str)
            .or(e.get("id").and_then(Value::as_str));
        let Some(id) = id.and_then(clean) else {
            continue;
        };
        let prefix = if e.pointer("/data/animated").and_then(Value::as_bool) == Some(true) {
            "7tva"
        } else {
            "7tv"
        };
        out.insert(name.to_string(), format!("{prefix}_{id}"));
    }
}

/// Badges Twitch (GraphQL) : `[{setID, version, imageURL}]` → `set/version` → `badge_UUID`.
pub fn parse_badges(list: &Value, out: &mut HashMap<String, String>) {
    for b in list.as_array().into_iter().flatten() {
        let (Some(set), Some(version), Some(url)) = (
            b["setID"].as_str(),
            b["version"].as_str(),
            b["imageURL"].as_str(),
        ) else {
            continue;
        };
        let Some(uuid) = url
            .strip_prefix("https://static-cdn.jtvnw.net/badges/v1/")
            .and_then(|r| r.split('/').next())
            .and_then(clean)
        else {
            continue;
        };
        out.insert(format!("{set}/{version}"), format!("badge_{uuid}"));
    }
}

/// Charge les données d'une chaîne (bloquant : à appeler depuis un fil). Chaque source qui échoue est ignorée.
pub fn fetch(login: &str, emotes: bool, badges: bool) -> ChatExtras {
    let mut out = ChatExtras::default();
    let Some((user_id, global_badges, channel_badges)) = crate::twitch::fetch_badges(login) else {
        return out;
    };
    if badges {
        parse_badges(&global_badges, &mut out.badges);
        parse_badges(&channel_badges, &mut out.badges); // ceux de la chaîne remplacent les globaux
    }
    if !emotes || clean(&user_id).is_none() {
        return out;
    }
    // Les six listes sont demandées en même temps ; elles sont ensuite fusionnées dans un ordre fixe :
    // global puis chaîne, FFZ puis BTTV puis 7TV (la dernière l'emporte en cas de nom identique).
    let urls = [
        "https://api.betterttv.net/3/cached/frankerfacez/emotes/global".to_string(),
        "https://api.betterttv.net/3/cached/emotes/global".to_string(),
        "https://7tv.io/v3/emote-sets/global".to_string(),
        format!("https://api.betterttv.net/3/cached/frankerfacez/users/twitch/{user_id}"),
        format!("https://api.betterttv.net/3/cached/users/twitch/{user_id}"),
        format!("https://7tv.io/v3/users/twitch/{user_id}"),
    ];
    let a = agent();
    let lists: Vec<Option<Value>> = std::thread::scope(|s| {
        let handles: Vec<_> = urls.iter().map(|u| s.spawn(|| get_json(&a, u))).collect();
        handles
            .into_iter()
            .map(|h| h.join().ok().flatten())
            .collect()
    });
    let mut map = HashMap::new();
    for (i, v) in lists.iter().enumerate() {
        let Some(v) = v else { continue };
        match i {
            0 | 3 => parse_ffz(v, &mut map),
            1 => parse_bttv(v, &mut map),
            2 => parse_7tv(v.get("emotes").unwrap_or(&Value::Null), &mut map),
            4 => {
                parse_bttv(v.get("sharedEmotes").unwrap_or(&Value::Null), &mut map);
                parse_bttv(v.get("channelEmotes").unwrap_or(&Value::Null), &mut map);
            }
            _ => parse_7tv(
                v.pointer("/emote_set/emotes").unwrap_or(&Value::Null),
                &mut map,
            ),
        }
    }
    out.emotes = map;
    out
}

/// Remplace, dans les morceaux de texte, chaque mot qui est le nom d'une emote externe.
pub fn apply(emotes: &HashMap<String, String>, segments: &mut Vec<Segment>) {
    if emotes.is_empty() {
        return;
    }
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    for seg in segments.drain(..) {
        let Segment::Text(text) = seg else {
            out.push(seg);
            continue;
        };
        let mut pending = String::new();
        for word in text.split_inclusive(' ') {
            let w = word.trim_end_matches(' ');
            match emotes.get(w) {
                Some(key) => {
                    if !pending.is_empty() {
                        out.push(Segment::Text(std::mem::take(&mut pending)));
                    }
                    out.push(Segment::Emote {
                        id: key.clone(),
                        name: w.to_string(),
                    });
                    pending.push_str(&word[w.len()..]);
                }
                None => pending.push_str(word),
            }
        }
        if !pending.is_empty() {
            out.push(Segment::Text(pending));
        }
    }
    *segments = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lecture_des_listes() {
        let mut m = HashMap::new();
        parse_bttv(
            &json!([{"id": "abc123", "code": "EZ", "animated": false}, {"id": "../x", "code": "Bad"}]),
            &mut m,
        );
        parse_ffz(&json!([{"id": 9, "code": "Zreh"}]), &mut m);
        parse_7tv(
            &json!([{"name": "Rain", "data": {"id": "01FC", "animated": true}}]),
            &mut m,
        );
        assert_eq!(m.get("EZ").map(String::as_str), Some("bttv_abc123"));
        assert_eq!(m.get("Zreh").map(String::as_str), Some("ffz_9"));
        assert_eq!(m.get("Rain").map(String::as_str), Some("7tva_01FC"));
        assert!(!m.contains_key("Bad"));
        let mut b = HashMap::new();
        parse_badges(
            &json!([
                {"setID": "subscriber", "version": "12", "imageURL": "https://static-cdn.jtvnw.net/badges/v1/e804d310-f35d/2"},
                {"setID": "x", "version": "1", "imageURL": "https://exemple.com/badges/v1/abc/2"}
            ]),
            &mut b,
        );
        assert_eq!(
            b.get("subscriber/12").map(String::as_str),
            Some("badge_e804d310-f35d")
        );
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn remplacement_par_mot() {
        let map: HashMap<String, String> = [("EZ".to_string(), "bttv_1".to_string())].into();
        let mut segs = vec![
            Segment::Text("EZ gg EZ".into()),
            Segment::Emote {
                id: "25".into(),
                name: "Kappa".into(),
            },
            Segment::Text(" pasEZ".into()),
        ];
        apply(&map, &mut segs);
        assert_eq!(
            segs,
            vec![
                Segment::Emote {
                    id: "bttv_1".into(),
                    name: "EZ".into()
                },
                Segment::Text(" gg ".into()),
                Segment::Emote {
                    id: "bttv_1".into(),
                    name: "EZ".into()
                },
                Segment::Emote {
                    id: "25".into(),
                    name: "Kappa".into()
                },
                Segment::Text(" pasEZ".into()),
            ]
        );
    }
}
