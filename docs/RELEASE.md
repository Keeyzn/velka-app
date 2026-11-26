# Publier une version

0. Une seule fois : `cargo run -p sign-release -- keygen <chemin hors du dépôt>`, puis commiter `assets/publication.pub`
   et sauvegarder la clé privée à 2 endroits hors de GitHub. La perdre empêche toute mise à jour future.
1. Changer `version` dans `Cargo.toml`, puis commiter.
2. `git tag vX.Y.Z` puis `git push --tags`.
3. Attendre la fin du workflow « Compilation » (onglet Actions) : il crée une Release brouillon avec les fichiers et
   `SHA256SUMS.txt`.
4. Signer : définir `VELKA_SIGNING_KEY_FILE` (chemin de la clé privée), puis `cargo run -p sign-release -- sign vX.Y.Z`
   (vérifie les empreintes, ajoute `velka-maj.json` à la Release après confirmation ; outil `gh` connecté).
5. Publier la Release brouillon à la main sur GitHub.

Workflow raté pour une étiquette : supprimer la Release brouillon créée (s'il y en a une), corriger, puis recréer
l'étiquette.
