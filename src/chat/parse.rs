//! Analyse des lignes IRC Twitch (fonctions pures, testées).
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct IrcMessage {
    pub tags: HashMap<String, String>,
    pub prefix: Option<String>,
    pub command: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Text(String),
    Emote { id: String, name: String },
    Link(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub user: String,
    /// Couleur Twitch `(r, g, b)` ; `None` si l'utilisateur n'en a pas choisi.
    pub color: Option<(u8, u8, u8)>,
    pub segments: Vec<Segment>,
    pub is_action: bool,
    pub is_reply: bool,
    pub is_mention: bool,
    /// Contient un des mots surlignés (réglage « Mots surlignés »).
    pub highlight: bool,
    /// Heure locale d'arrivée « HH:MM » (horodatage).
    pub time: String,
    /// Badges Twitch du message (`set/version`, dans l'ordre d'affichage).
    pub badges: Vec<String>,
}

/// Comptes de bots courants, masqués avec l'option « Masquer les bots ».
const BOTS: [&str; 12] = [
    "nightbot",
    "streamelements",
    "streamlabs",
    "moobot",
    "fossabot",
    "wizebot",
    "soundalerts",
    "commanderroot",
    "streamlabsbot",
    "sery_bot",
    "lurxx",
    "pretzelrocks",
];

pub fn is_bot(user: &str) -> bool {
    let u = user.to_lowercase();
    BOTS.contains(&u.as_str())
}

/// Le message contient-il un des mots de la liste (séparés par des virgules, sans tenir compte de la casse) ?
pub fn contains_word(words: &str, m: &ChatMessage) -> bool {
    let list: Vec<String> = words
        .split(',')
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if list.is_empty() {
        return false;
    }
    let text: String = m
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Text(t) => Some(t.to_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    list.iter().any(|w| text.contains(w.as_str()))
}

/// Décode les échappements des valeurs de tags (`\s`, `\:`, `\\`, `\r`, `\n`).
pub fn unescape_tag(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('s') => out.push(' '),
                Some(':') => out.push(';'),
                Some('r') => out.push('\r'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn parse_line(line: &str) -> Option<IrcMessage> {
    let mut rest = line.trim_end_matches(['\r', '\n']);
    let mut tags = HashMap::new();
    if let Some(r) = rest.strip_prefix('@') {
        let (t, r) = r.split_once(' ')?;
        for pair in t.split(';') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            tags.insert(k.to_string(), unescape_tag(v));
        }
        rest = r.trim_start();
    }
    let mut prefix = None;
    if let Some(r) = rest.strip_prefix(':') {
        let (p, r) = r.split_once(' ')?;
        prefix = Some(p.to_string());
        rest = r.trim_start();
    }
    let (head, trailing) = match rest.split_once(" :") {
        Some((h, t)) => (h, Some(t)),
        None => (rest, None),
    };
    let mut parts = head.split_whitespace();
    let command = parts.next()?.to_string();
    let mut params: Vec<String> = parts.map(str::to_string).collect();
    if let Some(t) = trailing {
        params.push(t.to_string());
    }
    Some(IrcMessage {
        tags,
        prefix,
        command,
        params,
    })
}

/// Intervalles d'emotes `(id, début, fin)` en indices de caractères, triés par début.
pub fn parse_emote_ranges(tag: &str) -> Vec<(String, usize, usize)> {
    let mut v = Vec::new();
    for entry in tag.split('/').filter(|e| !e.is_empty()) {
        let Some((id, ranges)) = entry.split_once(':') else {
            continue;
        };
        for r in ranges.split(',') {
            if let Some((a, b)) = r.split_once('-') {
                if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    if a <= b {
                        v.push((id.to_string(), a, b));
                    }
                }
            }
        }
    }
    v.sort_by_key(|r| r.1);
    v
}

/// Découpe le texte en segments : texte, emotes (par intervalles) et liens.
pub fn build_segments(text: &str, emotes: &[(String, usize, usize)]) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw: Vec<Segment> = Vec::new();
    let mut pos = 0;
    let push_text = |s: &str, raw: &mut Vec<Segment>| {
        for word in s.split_inclusive(' ') {
            let trimmed = word.trim_end();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                raw.push(Segment::Link(trimmed.to_string()));
                if word.len() > trimmed.len() {
                    raw.push(Segment::Text(" ".into()));
                }
            } else if !word.is_empty() {
                raw.push(Segment::Text(word.to_string()));
            }
        }
    };
    for (id, a, b) in emotes {
        // Intervalle incohérent (hors du texte ou chevauchant le précédent) : ignoré.
        let (Some(before), Some(name)) = (chars.get(pos..*a), chars.get(*a..=*b)) else {
            continue;
        };
        push_text(&before.iter().collect::<String>(), &mut raw);
        raw.push(Segment::Emote {
            id: id.clone(),
            name: name.iter().collect(),
        });
        pos = *b + 1;
    }
    push_text(
        &chars
            .get(pos..)
            .unwrap_or_default()
            .iter()
            .collect::<String>(),
        &mut raw,
    );

    // Fusionne les textes adjacents.
    let mut out: Vec<Segment> = Vec::new();
    for s in raw {
        match (out.last_mut(), s) {
            (Some(Segment::Text(prev)), Segment::Text(t)) => prev.push_str(&t),
            (_, s) => out.push(s),
        }
    }
    out
}

pub fn parse_color(tag: &str) -> Option<(u8, u8, u8)> {
    let h = tag.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn is_mention(text: &str, my_login: Option<&str>) -> bool {
    let Some(me) = my_login.filter(|m| !m.is_empty()) else {
        return false;
    };
    let needle = format!("@{}", me.to_lowercase());
    text.to_lowercase().contains(&needle)
}

/// Transforme un `PRIVMSG` en message de chat affichable.
pub fn to_chat_message(m: &IrcMessage, my_login: Option<&str>) -> Option<ChatMessage> {
    if m.command != "PRIVMSG" {
        return None;
    }
    let raw = m.params.get(1)?;
    let (text, is_action) = match raw.strip_prefix("\u{1}ACTION ") {
        Some(t) => (t.trim_end_matches('\u{1}'), true),
        None => (raw.as_str(), false),
    };
    let user = m
        .tags
        .get("display-name")
        .filter(|n| !n.is_empty())
        .cloned()
        .or_else(|| {
            m.prefix
                .as_ref()
                .and_then(|p| p.split('!').next().map(str::to_string))
        })?;
    let emotes = m
        .tags
        .get("emotes")
        .map(|e| parse_emote_ranges(e))
        .unwrap_or_default();
    Some(ChatMessage {
        user,
        color: m.tags.get("color").and_then(|c| parse_color(c)),
        segments: build_segments(text, &emotes),
        is_action,
        is_reply: m.tags.contains_key("reply-parent-msg-id"),
        is_mention: is_mention(text, my_login),
        highlight: false,
        time: String::new(),
        badges: parse_badges(m.tags.get("badges").map_or("", String::as_str)),
    })
}

/// Tag `badges` (`subscriber/12,premium/1`) : liste `set/version`, au plus 6.
pub fn parse_badges(tag: &str) -> Vec<String> {
    tag.split(',')
        .filter(|b| b.contains('/') && !b.starts_with('/'))
        .take(6)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn bots_et_mots_surlignes() {
        assert!(is_bot("Nightbot") && is_bot("StreamElements"));
        assert!(!is_bot("jean"));
        let m = ChatMessage {
            user: "jean".into(),
            color: None,
            segments: vec![Segment::Text("GG le Boss !".into())],
            is_action: false,
            is_reply: false,
            is_mention: false,
            highlight: false,
            time: String::new(),
            badges: Vec::new(),
        };
        assert!(contains_word("boss, autre", &m));
        assert!(!contains_word("  , ", &m));
        assert!(!contains_word("absent", &m));
        assert!(!contains_word("", &m));
    }

    use super::*;

    const LINE: &str = "@badge-info=;color=#FF4500;display-name=Jean\\sDupont;emotes=25:0-4,12-16/1902:6-10;reply-parent-msg-id=abc :jean!jean@jean.tmi.twitch.tv PRIVMSG #xqc :Kappa Keepo Kappa";

    #[test]
    fn ligne_privmsg() {
        let m = parse_line(LINE).unwrap();
        assert_eq!(m.command, "PRIVMSG");
        assert_eq!(m.params, vec!["#xqc", "Kappa Keepo Kappa"]);
        assert_eq!(m.tags["display-name"], "Jean Dupont");
        let c = to_chat_message(&m, None).unwrap();
        assert_eq!(c.user, "Jean Dupont");
        assert_eq!(c.color, Some((255, 69, 0)));
        assert!(c.is_reply);
        assert!(c.badges.is_empty());
        assert_eq!(
            parse_badges("subscriber/12,premium/1,,x"),
            vec!["subscriber/12".to_string(), "premium/1".to_string()]
        );
        assert_eq!(
            c.segments,
            vec![
                Segment::Emote {
                    id: "25".into(),
                    name: "Kappa".into()
                },
                Segment::Text(" ".into()),
                Segment::Emote {
                    id: "1902".into(),
                    name: "Keepo".into()
                },
                Segment::Text(" ".into()),
                Segment::Emote {
                    id: "25".into(),
                    name: "Kappa".into()
                },
            ]
        );
    }

    #[test]
    fn ping_et_sans_tags() {
        let m = parse_line("PING :tmi.twitch.tv\r\n").unwrap();
        assert_eq!(
            (m.command.as_str(), m.params[0].as_str()),
            ("PING", "tmi.twitch.tv")
        );
        let m = parse_line(":tmi.twitch.tv 001 justinfan1 :Welcome, GLHF!").unwrap();
        assert_eq!(m.command, "001");
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn emotes_unicode_et_liens() {
        // Les intervalles comptent des caractères, pas des octets.
        let segs = build_segments("é Kappa https://a.b/c fin", &parse_emote_ranges("25:2-6"));
        assert_eq!(segs[0], Segment::Text("é ".into()));
        assert_eq!(
            segs[1],
            Segment::Emote {
                id: "25".into(),
                name: "Kappa".into()
            }
        );
        assert!(segs.contains(&Segment::Link("https://a.b/c".into())));
    }

    #[test]
    fn intervalle_incoherent_ignore() {
        let segs = build_segments("abc", &parse_emote_ranges("25:5-9"));
        assert_eq!(segs, vec![Segment::Text("abc".into())]);
        // Début après la fin (envoyé par un serveur fautif) : ignoré, plus de plantage.
        let segs = build_segments("abcdef", &[("25".to_string(), 4, 1)]);
        assert_eq!(segs, vec![Segment::Text("abcdef".into())]);
    }

    #[test]
    fn action_couleur_mention() {
        let m =
            parse_line("@display-name=A :a!a@a PRIVMSG #c :\u{1}ACTION salut @Moi\u{1}").unwrap();
        let c = to_chat_message(&m, Some("moi")).unwrap();
        assert!(c.is_action && c.is_mention && c.color.is_none());
        assert_eq!(parse_color("#zzzzzz"), None);
        assert_eq!(parse_color("#abc"), None);
    }
}
