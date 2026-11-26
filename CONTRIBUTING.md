# Contribuer à Velka

Merci de votre intérêt ! Les contributions sont les bienvenues : signalements de bugs, idées, traductions et code.

## Avant de commencer
- Pour une modification importante, ouvrez d'abord une issue pour en discuter.
- Les contributions sont publiées sous la licence du projet, GPL-3.0-or-later.

## Développement
- Construire et lancer : voir `docs/build.md`. Architecture et règles de code : `docs/architecture.md` ;
  conventions d'interface : `DESIGN.md`.
- Avant d'ouvrir une pull request :
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
- Tout texte visible passe par `crate::i18n::tr("…")`, avec sa traduction anglaise dans `src/i18n.rs` et une ligne
  dans chaque fichier `src/lang/*.tsv` (les tests le vérifient).
- Pas d'`unwrap()` ni d'`expect()` hors des tests.

## Pull requests
- Une pull request = un sujet, avec une description claire de ce qui change et pourquoi.
- La CI (clippy + tests sous Windows et Linux) doit être verte.
