//! Normalisation du nom de chaîne Twitch.

/// Normalise une saisie (`xqc`, `@xqc`, `#xqc`, URL Twitch) en login valide.
/// Renvoie `None` si le résultat ne respecte pas `^[a-z0-9_]{1,25}$`.
pub fn normalize(input: &str) -> Option<String> {
    let mut s = input.trim().to_lowercase();
    for prefix in ["https://", "http://", "www.", "m."] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
        }
    }
    if let Some(rest) = s.strip_prefix("twitch.tv/") {
        s = rest.split(['/', '?', '#']).next().unwrap_or("").to_string();
    }
    let s = s.trim_start_matches(['@', '#']).to_string();
    let valid = !s.is_empty()
        && s.len() <= 25
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valid.then_some(s)
}

/// Formate un nombre de spectateurs : 950 → « 950 », 1234 → « 1,2 k ».
pub fn format_viewers(n: u64) -> String {
    format_viewers_in(n, !crate::i18n::uses_comma())
}

/// Même chose avec la langue précisée : « 1,2 k » en français, « 1.2 k » en anglais.
pub fn format_viewers_in(n: u64, english: bool) -> String {
    let sep = if english { '.' } else { ',' };
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1} k", n as f64 / 1000.0).replace('.', &sep.to_string())
    } else {
        format!("{:.1} M", n as f64 / 1_000_000.0).replace('.', &sep.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formes_valides() {
        for s in [
            "xqc",
            "@xqc",
            "#XQC",
            " xqc ",
            "https://www.twitch.tv/xqc",
            "twitch.tv/xqc/videos",
            "https://twitch.tv/xqc?a=1",
        ] {
            assert_eq!(normalize(s).as_deref(), Some("xqc"), "{s}");
        }
    }

    #[test]
    fn formes_invalides() {
        let long = "a".repeat(26);
        for s in ["", "   ", "@", "a b", "é", "a-b", long.as_str()] {
            assert_eq!(normalize(s), None, "{s}");
        }
        assert!(normalize(&"a".repeat(25)).is_some());
    }

    #[test]
    fn spectateurs() {
        assert_eq!(format_viewers(0), "0");
        assert_eq!(format_viewers(999), "999");
        assert_eq!(format_viewers(1234), "1,2 k");
        assert_eq!(format_viewers(25_000), "25,0 k");
        assert_eq!(format_viewers(1_500_000), "1,5 M");
        assert_eq!(format_viewers_in(1234, true), "1.2 k");
    }
}
