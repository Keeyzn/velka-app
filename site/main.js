// Site de Velka : langue (anglais par défaut, français si le navigateur est en français) et bouton de
// téléchargement adapté au système. Aucun traceur, aucun cookie : la langue choisie est seulement gardée dans le
// navigateur (localStorage).
(function () {
  "use strict";

  var FR = {
      "skip": "Aller au contenu",
      "nav.features": "Fonctionnalités",
      "nav.install": "Installation",
      "nav.trust": "Sécurité",
      "nav.faq": "Questions",
      "hero.badge": "Libre et gratuit · Windows, macOS, Linux",
      "hero.title": "Vos lives Twitch, en léger.",
      "hero.lead": "La vidéo et le chat dans une seule fenêtre, le multistream, un mini-lecteur et les notifications de vos favoris. Une vraie application, rapide et discrète.",
      "hero.download": "Télécharger",
      "hero.other": "Toutes les versions",
      "hero.fine": "Sans compte, sans publicité ajoutée, sans télémétrie. Non affilié à Twitch.",
      "features.title": "Tout ce qu'il faut, rien de plus",
      "f1.t": "Une seule fenêtre",
      "f1.d": "Vidéo et chat côte à côte, chat à gauche ou à droite, repliable en un clic.",
      "f2.t": "Multistream",
      "f2.d": "Jusqu'à 4 lives en même temps, avec le son de celui que vous choisissez.",
      "f3.t": "Mini-lecteur et PiP",
      "f3.d": "Le live continue dans une petite fenêtre, au premier plan si vous le voulez.",
      "f4.t": "Chat complet",
      "f4.d": "Émotes Twitch, 7TV, BTTV et FFZ, badges, et écriture dans le chat une fois connecté.",
      "f5.t": "Notifications",
      "f5.d": "Une alerte quand un favori passe en direct. Vos chaînes suivies, si vous vous connectez.",
      "f6.t": "À votre façon",
      "f6.d": "Thèmes clair et sombres, couleur d'accent, raccourcis, accessibilité, 25 langues.",
      "shots.title": "En images",
      "cap.player": "La vidéo et le chat, côte à côte.",
      "cap.multi": "Quatre lives à la fois avec le multistream.",
      "shots.note": "Captures de chaînes publiques prises au hasard ; le chat est flouté.",
      "install.title": "Installation",
      "install.sub": "Au premier lancement, cliquez sur « Tout installer » : Velka télécharge Streamlink et libmpv depuis leurs dépôts officiels, après vérification de leur empreinte.",
      "win.req": "Windows 10 ou 11, 64 bits",
      "win.btn": "Installateur (.exe)",
      "win.portable": "Version portable",
      "win.note": "« Windows a protégé votre ordinateur » : Informations complémentaires, puis Exécuter quand même.",
      "mac.req": "macOS 11 ou plus récent, Apple Silicon",
      "mac.btn": "Image disque (.dmg)",
      "mac.note": "Glissez Velka dans Applications. Premier lancement : clic droit sur Velka, puis Ouvrir.",
      "linux.req": "x86_64, X11 ou Wayland",
      "linux.btn": "AppImage",
      "linux.note": "Rendez le fichier exécutable, puis ouvrez-le :",
      "trust.title": "Ouvert et vérifiable",
      "trust.sub": "Le code source est public, sous licence GPL-3.0. Chaque version est compilée par GitHub Actions, à partir du dépôt.",
      "t1": "Aucune télémétrie, aucune statistique d'usage, aucune publicité ajoutée.",
      "t2": "Connexion Twitch facultative, par la page officielle de Twitch ; les jetons sont chiffrés sur votre ordinateur.",
      "t3": "Mises à jour vérifiées par une signature Ed25519 avant d'être installées.",
      "t4": "Empreintes SHA-256 publiées avec chaque version (SHA256SUMS.txt).",
      "t4.link": "Vérifier un fichier",
      "faq.title": "Questions",
      "q1": "Velka est-il vraiment gratuit ?",
      "a1": "Oui, et il le restera : c'est un logiciel libre. Vous pouvez soutenir le projet si vous le souhaitez, sans contrepartie.",
      "q2": "Pourquoi Windows ou macOS affichent-ils un avertissement ?",
      "a2": "Les fichiers ne sont pas signés par un certificat payant. Vous pouvez vérifier leur empreinte avec SHA256SUMS.txt avant de les ouvrir.",
      "q3": "Faut-il un compte Twitch ?",
      "a3": "Non. La connexion sert seulement à écrire dans le chat et à retrouver vos chaînes suivies.",
      "q4": "Velka est-il lié à Twitch ?",
      "a4": "Non. Velka est un projet indépendant, non affilié à Twitch. « Twitch » est une marque de Twitch Interactive, Inc.",
      "q5": "Comment désinstaller Velka ?",
      "a5": "Réglages, Avancé et Système, puis « Désinstaller » : l'application et ses données sont supprimées.",
      "support.title": "Soutenir le projet",
      "support.sub": "Velka est gratuit et sans publicité. Un don, sans contrepartie, aide à le faire vivre.",
      "support.btn": "Faire un don",
      "foot.license": "Logiciel libre sous licence GPL-3.0-or-later.",
      "foot.aff": "Non affilié à Twitch.",
      "foot.releases": "Versions",
      "foot.security": "Sécurité",
      "alt.home": "Accueil de Velka : favoris en direct avec leurs aperçus et le nombre de spectateurs",
      "alt.player": "Lecteur de Velka : vidéo du live et chat dans la même fenêtre",
      "alt.multi": "Multistream de Velka : quatre lives en même temps",
      "meta.title": "Velka — lecteur Twitch léger, libre et gratuit",
      "meta.desc": "Velka : lecteur Twitch léger pour Windows, macOS et Linux. Vidéo et chat dans une seule fenêtre, multistream, mini-lecteur, notifications. Libre et gratuit."
  };

  var nodes = document.querySelectorAll("[data-i18n]");
  var alts = document.querySelectorAll("[data-i18n-alt]");
  var meta = document.querySelector('meta[name="description"]');
  // Textes anglais : ceux de la page.
  var EN = { "meta.title": document.title, "meta.desc": meta ? meta.getAttribute("content") : "" };
  for (var i = 0; i < nodes.length; i++) EN[nodes[i].getAttribute("data-i18n")] = nodes[i].textContent;
  for (var j = 0; j < alts.length; j++) EN[alts[j].getAttribute("data-i18n-alt")] = alts[j].getAttribute("alt");

  var button = document.getElementById("lang");
  var current = "en";

  function apply(lang) {
    var table = lang === "fr" ? FR : EN;
    for (var i = 0; i < nodes.length; i++) {
      var key = nodes[i].getAttribute("data-i18n");
      if (table[key] !== undefined) nodes[i].textContent = table[key];
    }
    for (var j = 0; j < alts.length; j++) {
      var k = alts[j].getAttribute("data-i18n-alt");
      if (table[k] !== undefined) alts[j].setAttribute("alt", table[k]);
    }
    document.title = table["meta.title"];
    if (meta) meta.setAttribute("content", table["meta.desc"]);
    document.documentElement.lang = lang;
    current = lang;
    button.textContent = lang === "fr" ? "EN" : "FR";
    button.setAttribute("aria-label", lang === "fr" ? "English" : "Français");
    downloadButton();
  }

  // Bouton principal : fichier du système détecté (sinon page des versions).
  function detectOs() {
    var ua = navigator.userAgent || "";
    if (/Windows/i.test(ua)) return "windows";
    if (/Mac OS X|Macintosh/i.test(ua) && !/iPhone|iPad/i.test(ua)) return "mac";
    if (/Linux/i.test(ua) && !/Android/i.test(ua)) return "linux";
    return null;
  }

  var base = "https://github.com/Keeyzn/velka-app/releases/latest/download/";
  var files = {
    windows: { file: "velka-setup.exe", en: "Download for Windows", fr: "Télécharger pour Windows" },
    mac: { file: "Velka-mac.dmg", en: "Download for macOS", fr: "Télécharger pour macOS" },
    linux: { file: "Velka-x86_64.AppImage", en: "Download for Linux", fr: "Télécharger pour Linux" }
  };
  var os = detectOs();

  function downloadButton() {
    if (!os) return;
    document.getElementById("dl-main").href = base + files[os].file;
    document.getElementById("dl-main-label").textContent = files[os][current];
  }

  if (os) {
    var card = document.querySelector('.os[data-os="' + os + '"]');
    if (card) card.classList.add("mine");
  }

  // Français si la langue principale du navigateur est le français ; sinon anglais. Un choix fait avec le bouton
  // est gardé.
  function browserIsFrench() {
    var first = (navigator.languages && navigator.languages[0]) || navigator.language || "";
    return first.toLowerCase().indexOf("fr") === 0;
  }
  var saved = null;
  try { saved = localStorage.getItem("velka-lang"); } catch (e) { /* stockage indisponible */ }
  apply(saved === "fr" || saved === "en" ? saved : (browserIsFrench() ? "fr" : "en"));

  button.addEventListener("click", function () {
    var next = current === "fr" ? "en" : "fr";
    apply(next);
    try { localStorage.setItem("velka-lang", next); } catch (e) { /* stockage indisponible */ }
  });
})();
