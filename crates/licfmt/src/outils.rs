//! Composants tiers (Streamlink, libmpv) : dépôts GitHub et versions validées, inscrites dans le code. Seule une
//! archive dont le nom et l'empreinte SHA-256 figurent ici est installée. Pour passer à une nouvelle version d'un
//! composant : modifier `PINNED` (étiquette, nom exact de l'archive, empreinte), puis publier une version de Velka.

pub const STREAMLINK: &str = "streamlink";
pub const LIBMPV: &str = "libmpv";
/// Composants Mac : archives fabriquées pour Velka (scripts de fabrication et sources dans le dépôt `MACOS_REPO`).
pub const STREAMLINK_MACOS: &str = "streamlink-macos";
pub const LIBMPV_MACOS: &str = "libmpv-macos";
/// Dépôt public des archives Mac fabriquées pour Velka.
pub const MACOS_REPO: &str = "Keeyzn/velka-composants-mac";
/// Composants Linux : archives fabriquées pour Velka (scripts de fabrication et sources dans le dépôt `LINUX_REPO`).
pub const STREAMLINK_LINUX: &str = "streamlink-linux";
pub const LIBMPV_LINUX: &str = "libmpv-linux";
/// Dépôt public des archives Linux fabriquées pour Velka.
pub const LINUX_REPO: &str = "Keeyzn/velka-composants-linux";

/// Dépôt GitHub d'un composant (nom court `streamlink` / `libmpv`).
pub fn repo(outil: &str) -> Option<&'static str> {
    match outil {
        STREAMLINK => Some("streamlink/windows-builds"),
        LIBMPV => Some("zhongfly/mpv-winbuild"),
        STREAMLINK_MACOS | LIBMPV_MACOS => Some(MACOS_REPO),
        STREAMLINK_LINUX | LIBMPV_LINUX => Some(LINUX_REPO),
        _ => None,
    }
}

/// Une version validée : étiquette de la release GitHub, nom exact de l'archive et son empreinte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinned {
    pub outil: String,
    pub tag: String,
    pub asset: String,
    /// SHA-256 en hexadécimal minuscule.
    pub sha256: String,
}

/// Versions validées : (composant, étiquette, archive, SHA-256). libmpv est toujours un build LGPL
/// (`-Dgpl=false`, FFmpeg sans `--enable-gpl`).
const PINNED: &[(&str, &str, &str, &str)] = &[
    (
        STREAMLINK,
        "8.6.3-1",
        "streamlink-8.6.3-1-py315-x86_64.zip",
        "3b1f1470d4b135080a8107ca2917379ca3ff5acadbcdc215568b79eea1450497",
    ),
    (
        LIBMPV,
        "2026-11-24-35efcf53d5",
        "mpv-dev-lgpl-x86_64-20261124-git-35efcf53d5.7z",
        "9fbf5d6df03b914fa211f884685cf072b989840c0084f9dddcd33164de8e03f0",
    ),
    (
        STREAMLINK_MACOS,
        "2026-11-25",
        "streamlink-8.6.3-macos-arm64.zip",
        "5c1da1828d6e240d723e632f5644a2eb6b7be23bdaa05b3d090368722f264b91",
    ),
    (
        LIBMPV_MACOS,
        "2026-11-25",
        "libmpv-lgpl-v0.7.3-macos-arm64.zip",
        "14971ab283d96d602a9e7f3bf72857d4a8d742adaf2e23577d84c512d546f018",
    ),
    (
        STREAMLINK_LINUX,
        "2026-11-25",
        "streamlink-8.6.3-linux-x86_64.zip",
        "290582622f1511565457cd2a3c25ae242ebaaad9c342a31939838cd1a4051a32",
    ),
    (
        LIBMPV_LINUX,
        "2026-11-25",
        "libmpv-lgpl-v0.42.0-linux-x86_64.zip",
        "897a095bf1460487eba30f8db72337ee148948e1121fb0d8dfc4f5246f918eb0",
    ),
];

/// Version validée d'un composant ; `None` si le composant est inconnu.
pub fn pinned(outil: &str) -> Option<Pinned> {
    PINNED
        .iter()
        .find(|p| p.0 == outil)
        .map(|&(outil, tag, asset, sha256)| Pinned {
            outil: outil.into(),
            tag: tag.into(),
            asset: asset.into(),
            sha256: sha256.into(),
        })
}

#[cfg(test)]
fn safe_field(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
        && !s.starts_with('.')
}

#[cfg(test)]
fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// L'adresse de téléchargement d'une archive doit être celle des releases du dépôt attendu, en HTTPS.
pub fn download_url_ok(outil: &str, url: &str) -> bool {
    match repo(outil) {
        Some(r) => url
            .strip_prefix(&format!("https://github.com/{r}/releases/download/"))
            .is_some_and(|rest| !rest.contains("..") && !rest.contains('?') && !rest.contains('#')),
        None => false,
    }
}

/// Choisit l'archive de développement libmpv **LGPL** (`mpv-dev-lgpl-x86_64-*.7z`, sans `v3`) : build
/// `-Dgpl=false`, FFmpeg sans `--enable-gpl`. Aucun repli sur le build GPL.
pub fn pick_libmpv_lgpl(names: &[&str]) -> Option<usize> {
    let ok = |n: &str, allow_v3: bool| {
        let n = n.to_lowercase();
        n.starts_with("mpv-dev-lgpl-x86_64")
            && n.ends_with(".7z")
            && (allow_v3 || !n.contains("v3"))
    };
    names
        .iter()
        .position(|n| ok(n, false))
        .or_else(|| names.iter().position(|n| ok(n, true)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_validees_bien_formees() {
        for &(outil, tag, asset, sha) in PINNED {
            assert!(repo(outil).is_some(), "{outil} : dépôt inconnu");
            assert!(
                safe_field(tag) && safe_field(asset),
                "{outil} : étiquette ou nom d'archive invalide"
            );
            assert!(is_sha256(sha), "{outil} : empreinte invalide");
            assert_eq!(
                PINNED.iter().filter(|p| p.0 == outil).count(),
                1,
                "{outil} en double"
            );
        }
        for outil in [
            STREAMLINK,
            LIBMPV,
            STREAMLINK_MACOS,
            LIBMPV_MACOS,
            STREAMLINK_LINUX,
            LIBMPV_LINUX,
        ] {
            assert!(pinned(outil).is_some(), "{outil} sans version validée");
        }
        assert!(pinned("autre").is_none());
        let libmpv = pinned(LIBMPV).map(|p| p.asset).unwrap_or_default();
        assert_eq!(
            pick_libmpv_lgpl(&[libmpv.as_str()]),
            Some(0),
            "libmpv Windows : build LGPL"
        );
        for outil in [LIBMPV_MACOS, LIBMPV_LINUX] {
            assert!(
                pinned(outil).is_some_and(|p| p.asset.starts_with("libmpv-lgpl-")),
                "{outil} : build LGPL"
            );
        }
    }

    #[test]
    fn adresse_de_telechargement() {
        let ok = "https://github.com/streamlink/windows-builds/releases/download/8.6.3-1/a.zip";
        assert!(download_url_ok(STREAMLINK, ok));
        assert!(!download_url_ok(LIBMPV, ok), "autre dépôt");
        assert!(!download_url_ok(
            STREAMLINK,
            &ok.replace("https://", "http://")
        ));
        assert!(!download_url_ok(
            STREAMLINK,
            "https://evil.example/streamlink/windows-builds/releases/download/a.zip"
        ));
        assert!(!download_url_ok(
            STREAMLINK,
            "https://github.com/streamlink/windows-builds/releases/download/../../x"
        ));
        assert!(!download_url_ok(
            STREAMLINK,
            "https://github.com.evil.example/streamlink/windows-builds/releases/download/a"
        ));
        assert!(download_url_ok(
            LIBMPV_MACOS,
            "https://github.com/Keeyzn/velka-composants-mac/releases/download/2026-11-25/a.zip"
        ));
        assert!(!download_url_ok(
            LIBMPV_LINUX,
            "https://github.com/Keeyzn/velka-composants-mac/releases/download/x/y.zip"
        ));
    }

    #[test]
    fn choix_libmpv_lgpl() {
        let names = [
            "mpv-dev-x86_64-20261122-git-c2a42f414f.7z",
            "mpv-dev-lgpl-aarch64-20261122-git-c2a42f414f.7z",
            "mpv-dev-lgpl-x86_64-v3-20261122-git-c2a42f414f.7z",
            "mpv-dev-lgpl-x86_64-20261122-git-c2a42f414f.7z",
        ];
        assert_eq!(pick_libmpv_lgpl(&names), Some(3));
        assert_eq!(
            pick_libmpv_lgpl(&names[..3]),
            Some(2),
            "repli sur v3, jamais sur le build GPL"
        );
        assert_eq!(pick_libmpv_lgpl(&names[..2]), None);
    }
}
