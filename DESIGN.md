# Velka — conventions d'interface et d'accessibilité

Charte : dark premium, bordures hairline, une seule couleur d'accent (orange par défaut, `#CA4F24`).
Interface **native egui** : WCAG 2.2 AA appliqué via WCAG2ICT, sans CSS ni ARIA. Tout passe par `src/ui/theme.rs`.

## Couleurs
- Jamais de couleur codée en dur dans un écran : fonctions `t::TEXT()`, `t::MUTED()`, `t::FAINT()`, `t::POSITIVE()`,
  `t::NEGATIVE()`, `t::FOCUS()`… (elles suivent le thème Dark / Sombre / Clair : « Dark » = noir, valeur 0 ; « Sombre » = bleu nuit, valeur 1).
- Texte : **4,5:1 minimum** sur `bg`, `surface-1` et `surface-2` dans les trois thèmes, verrouillé par le test
  `contrastes_du_texte_aa`. `FAINT` reste le niveau le plus discret, mais il est lisible (aides, listes vides, placeholders).
- Couleurs venues de l'extérieur (pseudos du chat) : passer par `t::readable_on(couleur, fond)`.
- Surcouches sur la vidéo : `t::DarkScope::enter(ui)` (toujours sombres, quel que soit le thème).

| Élément (thème) | Avant | Après | Ratio avant → après (sur bg / s1 / s2) | Seuil |
|---|---|---|---|---|
| `FAINT` (Sombre) | 85,85,92 | 127,127,134 | 2,7 / 2,6 / 2,4 → 5,0 / 4,8 / 4,6 | 4,5:1 |
| `FAINT` (Dark) | 96,104,121 | 130,138,155 | 3,3 / 3,1 / 2,8 → 5,3 / 5,0 / 4,5 | 4,5:1 |
| `FAINT` (Clair) | 112,117,132 | 98,103,118 | 4,0 / 4,6 / 3,7 → 4,9 / 5,6 / 4,6 | 4,5:1 |
| `POSITIVE` (Clair) | 21,128,61 | 13,120,53 | 4,3 / 5,0 / 4,1 → 4,8 / 5,6 / 4,5 | 4,5:1 |
| Anneau de focus | (aucun) | variante lisible de l'accent (`t::ACCENT_TEXT()`) | ≥ 3:1 sur toutes les surfaces | 3:1 |

Exceptions assumées : bordures hairline (décoratives, la charte le veut ; les contrôles se repèrent par leur fond, leur
texte et leur position) ; texte des contrôles désactivés (exempté par WCAG 1.4.3).

## Clavier et focus
- Tab / Maj+Tab parcourt tout ; Espace ou Entrée active. Un clic place aussi le focus (`t::focus_follows_click`,
  sauf sur la page du lecteur où Espace et les flèches sont des raccourcis) : Tab repart de l'élément cliqué.
- L'anneau ne s'affiche qu'après une touche de navigation, pas après un clic (`t::track_focus_modality`,
  appelé au début de chaque image).
- Tout composant dessiné à la main avec `Sense::click()` appelle **`t::focus_ring(ui, &resp, rect, rayon)`** et nomme
  l'élément avec `resp.widget_info(...)` (bouton, case, curseur). Les boutons egui standard ont déjà leur indicateur.
- Infobulles (`t::show_tip`) : s'ouvrent aussi au focus clavier, au-dessus du composant.
- **Défilement automatique au clavier** : toute zone défilante appelle `t::follow_keyboard_focus(ui)` au début de son contenu (Réglages, accueil, barre latérale) ; quand le focus change après une touche de navigation, la zone défile pour montrer l'élément (egui ne le fait pas seul). Pas d'effet après un clic. Toute nouvelle `ScrollArea` verticale doit l'appeler.
- Réglages = fenêtre **modale** pour le clavier : `Memory::set_modal_layer` (le voile est la couche la plus basse autorisée) ; Tab ne sort jamais vers la page du dessous, et à l'ouverture le focus est placé sur le champ de recherche (`settings_focus_frames`, appliqué après `focus_follows_click`). Le voile se clique mais n'est pas un arrêt de Tab (`Sense::CLICK`).
- Curseurs (`volume_slider`, `value_slider`) : flèches gauche / droite = un pas, Origine / Fin = bornes.
- Contrôles segmentés dans une ligne alignée à droite : mesurés puis dessinés de gauche à droite (ordre de Tab = ordre visuel).

## Mouvement, échelle, tailles
- Animations : `t::anim_secs` (0 si le réglage « Animations » est coupé ou si Windows désactive les animations).
- Agrandissement : Ctrl + / Ctrl − / Ctrl 0 (zoom egui) ou Réglages > Apparence > « Taille de l'interface »
  (80 à 150 %, `ui_zoom`, mémorisé ; Ctrl +/− met aussi le réglage à jour). Le curseur applique la taille au relâchement
  (au clavier : tout de suite). Les listes se réorganisent.
- **Contraste élevé** (`high_contrast` : 0 Automatique = suit le thème de contraste de Windows, 1 Activé, 2 Désactivé) :
  palettes `*_HC` (mêmes surfaces, texte et valeurs positive/négative ≥ 7:1, bordures ≥ 3:1), verrouillé par le test
  `contraste_eleve_7_a_1`. Toute nouvelle couleur de texte doit exister dans les trois palettes `*_HC`.
- **Focus clavier renforcé** (`focus_strong`) : anneau de `t::focus_width()` (3,5 px au lieu de 2), double contour,
  visible dès qu'un élément a le focus (même après un clic). Tout anneau dessiné à la main utilise `t::focus_width()`.
- `app::apply_access` applique contraste, focus, couleur d'accent et zoom (démarrage, Réglages, réinitialisation).
- **Couleurs** (onglet Réglages > Accessibilité, carte « Couleurs ») : `palette` (0 à 4 =
  `theme::PALETTES`, 5 = personnalisée, orange par défaut) et `accent_custom` (RVB). L'accent n'est plus une constante : `t::ACCENT()`
  (fond des boutons, texte blanc dessus), `t::ACCENT_HOVER()`, `t::ACCENT_TEXT()` (texte, lien, anneau), `t::ACCENT_LINE()`
  (contours), `t::ACCENT_GFX()`, `t::ACCENT_SOFT()`. Sans couleur choisie, l'accent est l'orange par défaut.
  Une couleur choisie doit donner **4,5:1 avec le texte blanc** (7:1 en contraste élevé) : sinon elle n'est pas appliquée,
  le texte l'explique et propose `t::nearest_compliant_fill` (même teinte, assombrie). Les variantes pour texte et
  traits sont dérivées automatiquement pour tenir le minimum sur les fonds de chaque thème (`compute_accent`) ;
  l'accent s'adapte donc au thème Système et au contraste élevé. Tests : `palettes_conformes`,
  `alternative_conforme_la_plus_proche`, `accent_par_defaut_orange`. Le verdict est toujours écrit en toutes lettres.
- Gabarits : lignes de réglage 38 px (50 avec précision), boutons 32 px (constantes `ROW_H`, `ROW_H_HINT`, `BTN_H` de `theme.rs`), texte courant 14–14,5 px, secondaire 11,5–12,5 px.

## Fenêtre, thème et langues
- Fenêtre sans bordure Windows : barre de titre de 36 px dessinée par l'app (boutons 46 × 36, survol gris, Fermer rouge
  `#C42B1C` comme Windows 11), bords redimensionnables, coins arrondis par Windows 11.
- Thème « Système » : suit Windows (clair → Clair, sombre → Sombre), choisi par défaut à la première installation.
- Réglages : overlay centré sur un voile noir à 65 %, menu de 236 px à gauche (surface-1, entrées texte `t::menu_item`),
  contenu à droite, bouton rond de fermeture avec « Échap » dessous (`t::close_button`).
- Langue : liste de lignes de 46 px (`t::choice_row`) : radio à gauche (rempli + liseré d'accent quand choisi), nom dans la
  langue, nom traduit à droite, pas de drapeaux. Chinois, japonais et coréen utilisent les polices « UI » de Windows.
- Menu de résultats de la recherche : posé exactement sous le champ, même largeur (`search_input` renvoie le cadre).

## Lecteur d'écran
- Fonction `accesskit` d'eframe activée (Narrateur, NVDA via UI Automation ; environ +0,3 Mo sur l'exe).
- Composants maison nommés avec `resp.widget_info(...)` ; élément sans texte visible : `t::a11y_label`.
- Dans `control_row`, segments et curseurs prennent le libellé de la ligne (« Qualité par défaut : 720p », boutons radio).
- Vérifié par UI Automation (liste des éléments exposés) ; pas encore essayé à l'oreille avec Narrateur.
