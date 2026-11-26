# Distribution de Velka

## Pourquoi les exécutables ne sont pas signés
Velka est gratuit. Un certificat de signature de code (Authenticode pour Windows, compte Apple Developer pour macOS)
est payant chaque année : les fichiers publiés n'en ont pas pour l'instant. Windows (SmartScreen) et macOS (Gatekeeper)
affichent donc un avertissement au premier lancement. L'application elle-même vérifie ses mises à jour avec une
signature Ed25519 (voir plus bas).

## Vérifier l'intégrité d'un fichier
Chaque Release contient `SHA256SUMS.txt` (empreintes calculées par GitHub Actions) et `velka-maj.json` (empreintes et
signatures Ed25519 des fichiers de mise à jour, faites avec la clé dont la partie publique est `assets/publication.pub`).

- Windows (PowerShell) : `Get-FileHash .\velka-setup.exe -Algorithm SHA256`
- macOS : `shasum -a 256 Velka-mac.dmg`
- Linux : `sha256sum -c SHA256SUMS.txt --ignore-missing`

Comparez le résultat avec la ligne correspondante de `SHA256SUMS.txt`. Les mises à jour automatiques font ce contrôle
seules : un fichier dont la signature n'est pas valide est refusé.

## Windows : avertissement SmartScreen
« Windows a protégé votre ordinateur » : cliquez sur **Informations complémentaires**, puis **Exécuter quand même**.
Sur un PC où le « Contrôle intelligent des applications » est activé, un programme non signé peut être bloqué sans
cette option.

## macOS : avertissement Gatekeeper
Faites un **clic droit** (ou Ctrl-clic) sur Velka dans Applications, choisissez **Ouvrir**, puis confirmez. Si macOS
refuse encore : Réglages Système > Confidentialité et sécurité > « Ouvrir quand même ».

## Faux positif d'un antivirus
Un programme récent et non signé est parfois signalé à tort. Vérifiez d'abord l'empreinte (ci-dessus), puis signalez le
fichier à Microsoft comme faux positif : https://www.microsoft.com/wdsi/filesubmission (« Software developer » pour le
mainteneur, « Home customer » pour un utilisateur). Pour un autre antivirus, utilisez le formulaire de faux positif de
son éditeur.
