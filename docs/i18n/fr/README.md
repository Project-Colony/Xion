<!-- xion-i18n: source=README.md sha=8b82fe4b0e2baf73ca5d4db01f717baec48c5a6d -->

<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../../../assets/brand/png/xion-logo-512.png">
  <img src="../../../assets/brand/png/xion-logo-light-1024.png" alt="Xion" width="360">
</picture>

**Un explorateur de fichiers à onglets, avec un vrai panneau terminal, identique sous Linux et Windows.**

</div>

[![Licence : GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](../../../LICENSE)
[![Application Colony](https://img.shields.io/badge/Colony-utility-purple)](https://github.com/Project-Colony/Colony)
[![Plateformes](https://img.shields.io/badge/platforms-linux%20%7C%20windows-lightgrey)](#installation)

Chaque bureau livre son propre gestionnaire de fichiers : Nautilus sous GNOME,
Dolphin sous KDE, l'Explorateur sous Windows. Chacun appartient à sa
plateforme, si bien que changer de machine veut dire changer d'outil, de
raccourcis et d'habitudes. Xion est un seul explorateur pour Linux et Windows,
avec des onglets, des aperçus, des archives et un panneau terminal, écrit en
Rust sur la boîte à outils que partagent tous les programmes de
[Project Colony](https://github.com/Project-Colony).

> **État :** jeune. Prouvé : chaque modification est compilée et testée sous
> Linux et Windows en CI, ainsi que sur la version minimale de Rust ;
> `cargo deny` ne signale aucun avis ouvert ; les versions sont publiées
> signées, et les signatures se vérifient. Branché mais non testé :
> l'intégration au menu contextuel de Windows (`xion --register`) n'a été
> essayée qu'à la main, et le passage de relais à l'instance unique n'existe
> que sous Linux (via D-Bus) : sous Windows, chaque lancement ouvre une
> nouvelle fenêtre.

## Pourquoi Xion

- **Un seul programme sur les deux systèmes.** La même interface, les mêmes
  raccourcis et le même fichier de réglages sous Linux et sous Windows, au
  lieu d'un gestionnaire de fichiers différent sur chacun.
- **Un vrai terminal à côté des fichiers.** Le panneau du bas s'ouvre dans le
  dossier affiché, a ses propres onglets, et c'est un émulateur de terminal
  complet : `vim`, `htop` et Ctrl+C s'y comportent comme partout ailleurs.
- **Les archives sans autre outil.** Les archives ZIP, `.tar.gz` et 7z se
  parcourent comme des dossiers, s'extraient et se créent sur place, et
  l'extraction refuse toute entrée qui atterrirait hors de la destination.
- **Membre d'un écosystème.** Thèmes, accents, taille du texte, police adaptée
  à la dyslexie et animations réduites viennent du même catalogue et de la
  même disposition des Préférences que dans tous les autres programmes Colony.

## Ce qu'il fait

- Onglets, historique précédent et suivant, et une barre d'adresse modifiable
  avec suggestions.
- Une barre latérale avec les lecteurs, les favoris, l'accès rapide, les
  emplacements récents et une arborescence des dossiers, et un second volet
  facultatif.
- Vues en liste et en grille avec des colonnes redimensionnables et triables,
  un filtre rapide, et des étiquettes de couleur sur les fichiers.
- Copier, déplacer, renommer (un fichier ou plusieurs à la fois) et supprimer,
  avec progression et annulation de la dernière action. La suppression passe
  par la corbeille ; Maj+Suppr supprime définitivement après confirmation.
- Archives : parcourir, extraire et créer des `.zip`, `.tar.gz` et `.7z`.
- Recherche par nom dans un index en mémoire du dossier, et une recherche
  plein texte récursive qui peut respecter `.gitignore`.
- Aperçus : métadonnées, images, texte avec coloration syntaxique, vue
  hexadécimale, et différence entre deux fichiers. Les propriétés affichent le
  SHA-256 d'un fichier et permettent de modifier ses permissions.
- Dans un dépôt git, les fichiers modifiés, nouveaux, indexés et en conflit
  sont signalés dans la liste.
- Un panneau terminal intégré, construit sur `iced_term`
  (`alacritty_terminal`), qui lance votre propre shell.
- Les emplacements montés par gvfs (SMB, SFTP, téléphones, Google Drive) dans
  la barre latérale, et les services SMB, FTP, HTTP et d'impression qu'annonce
  votre réseau local.
- Une interface en anglais et en français. Préférences > Général > Langue en
  choisit une, ou suit par défaut la langue de la session.
- Des préférences d'apparence et d'accessibilité partagées avec l'écosystème
  Colony : tous les thèmes et accents du catalogue Colony, le contraste élevé,
  deux tailles de texte qui se multiplient, la police OpenDyslexic, et les
  animations réduites, qui coupent toute animation. Le bouton **Xion** en haut
  à gauche ouvre les Préférences.
- Les réglages dans un seul fichier TOML versionné sous `Colony/Xion/`,
  enregistrés à chaque modification et rechargés avec Ctrl+R.

## Installation

### Via Colony (recommandé)

Cherchez **Xion** dans [Colony](https://github.com/Project-Colony/Colony) et
installez-le. Les mises à jour arrivent par le lanceur, qui vérifie la
signature de chaque version avant de l'installer.

### Téléchargement direct du binaire

Récupérez le fichier de votre plateforme dans la
[dernière version](https://github.com/Project-Colony/Xion/releases/latest) :

| Plateforme | Fichier |
|---|---|
| Linux | `xion-linux` |
| Windows | `xion-windows.exe` |

```bash
chmod +x xion-linux && ./xion-linux
```

Pour vérifier d'abord le téléchargement, voir la
[politique de signature du code](#politique-de-signature-du-code).

### Compiler depuis les sources

```bash
git clone https://github.com/Project-Colony/Xion
cd Xion
cargo build --release
./target/release/xion
```

Demande Rust 1.95 ou plus récent ; `rust-toolchain.toml` épingle la chaîne
d'outils qu'utilise la CI, et rustup l'installe à la première compilation.
Au-delà de Rust, la compilation ne demande qu'un compilateur C
(`build-essential` sous Debian ou Ubuntu) pour le code C embarqué de libgit2,
zlib et zstd. Les bibliothèques de fenêtrage sont chargées à l'exécution :
aucun paquet de développement n'est nécessaire.

Sous Linux, `scripts/install.sh` compile un binaire optimisé et l'installe
avec ses icônes et son entrée de bureau sous `~/.local`, sans `sudo`.
`scripts/install.sh --uninstall` retire exactement ce qu'il a installé.

## Documentation

La documentation complète est dans [docs/](../../../docs/README.md), en
anglais ; son index est traduit : [docs/README.md](docs/README.md). Pour
signaler une vulnérabilité, voir [SECURITY.md](../../../SECURITY.md) (en
anglais).

## Langue

Certaines pages sont traduites. **L'anglais fait foi** : quand une traduction
le contredit, c'est le fichier anglais qui a raison.

- **English** - [README](../../../README.md) · [documentation](../../../docs/README.md) · [goals and decisions](../../../docs/project/decisions.md)

## Politique de signature du code

Chaque fichier publié est signé par l'organisation Project Colony en CI,
jamais sur une machine de développement. À côté de chaque fichier, sur la page
de la version :

| Fichier | Ce que c'est |
|---|---|
| `<asset>.sig` | une signature ed25519 du fichier, faite avec la clé de publication de l'organisation |
| `<asset>.meta` | trois lignes qui lient le fichier à son nom, à son sha256 et à la version publiée |
| `<asset>.meta.sig` | une signature ed25519 du `.meta` |

La clé privée est un secret de l'organisation, utilisé seulement par le
[workflow partagé de signature et de publication](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/.github/workflows/sign-and-publish.yml),
dans un job qui ne compile rien ; les jobs qui compilent Xion ne la voient
jamais. Colony vérifie les trois fichiers avant d'installer ou de mettre à
jour Xion, et refuse une version plus ancienne que celle installée. Pour
vérifier vous-même un téléchargement avec OpenSSL 3 (les mêmes commandes
valent pour chaque fichier) :

```bash
cat > colony-release.pub <<'EOF'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEARNjg3Nn8H6/aBg1unwGjkUTcrdTxERNefVaqU8cFu0s=
-----END PUBLIC KEY-----
EOF
a=xion-linux
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a" -sigfile "$a.sig"
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a.meta" -sigfile "$a.meta.sig"
cat "$a.meta"     # version=<tag>, asset=<nom du fichier>, sha256=<condensat>
sha256sum "$a"    # le condensat doit être celui de la ligne sha256
```

Comment les versions sont compilées, signées et publiées :
[design/releases.md](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/design/releases.md#5-signing)
(en anglais).

## Vie privée

Xion n'envoie ni télémétrie, ni statistiques, ni rapports de plantage. Il n'a
pas de compte et ne cherche pas de mises à jour : c'est Colony qui s'en
charge.

| Donnée | Stockée ou envoyée | Où, et pourquoi |
|---|---|---|
| Réglages | stockés | `Colony/Xion/config.toml` dans le dossier de configuration de cette machine : vos préférences, favoris, étiquettes de couleur et largeurs de colonnes |
| Onglets ouverts | stockés | `Colony/Xion/session.toml` dans le dossier de données de cette machine, pour les rouvrir au prochain démarrage |
| Exploration du réseau | envoyée, sur votre réseau local uniquement | quand vous ouvrez ou actualisez *Réseau* dans la barre latérale, au plus toutes les 30 secondes : les requêtes mDNS standard pour les services SMB, FTP, HTTP et d'impression, pendant quelques secondes. Sous Windows, Xion demande aussi au système les partages SMB qu'il voit. Ce qui répond est listé, marqué comme non vérifié |
| Menu contextuel de Windows | stocké | des entrées sous `HKEY_CURRENT_USER\Software\Classes`, seulement après `xion --register` ; `xion --unregister` les retire |

Xion ne se connecte à aucun serveur, et ne touche votre réseau local que pour
l'exploration ci-dessus. Il ne demande ni ne stocke aucun mot de passe. Les
miniatures et l'index de recherche restent en mémoire. Il lance pour vous des
programmes locaux : votre shell dans le panneau terminal, `ffmpeg`, `mutool`
ou `pdftoppm` pour les miniatures de vidéos et de PDF quand ils sont
installés, et l'ouvreur du système pour les fichiers que vous ouvrez. Ouvrir
un partage réseau ou un emplacement gvfs est un accès ordinaire aux fichiers,
par le système.

## Licence

GPL-3.0-or-later. Voir [LICENSE](../../../LICENSE). Les polices de Xion
(JetBrains Mono Nerd Font, OpenDyslexic et Font Awesome 6 Free) sont
embarquées dans [colony-ui](https://crates.io/crates/colony-ui), qui les livre
avec le texte de leurs licences.
