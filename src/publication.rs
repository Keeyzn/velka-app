//! Clé publique de publication : elle vérifie la signature de chaque mise à jour de l'application (manifeste
//! `velka-maj.json` des Releases GitHub). La clé privée correspondante n'est jamais dans le dépôt ni dans GitHub : le
//! mainteneur signe sur son PC avec `tools/sign-release`. Fichier vide ou invalide : rien n'est accepté.

pub fn public_key() -> [u8; 32] {
    licfmt::public_key_from_hex(include_str!("../assets/publication.pub")).unwrap_or([0; 32])
}
