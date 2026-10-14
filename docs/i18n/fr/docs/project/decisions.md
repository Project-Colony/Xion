<!-- xion-i18n: source=docs/project/decisions.md sha=8973ce1cb6b396b841fd78d362cd591f3f362c4d -->

# Objectifs et décisions

## But du projet

Un explorateur de fichiers pour Linux et Windows, inspiré de Windows File
Explorer, Files et Dolphin : onglets, barre latérale, vues en liste et en
grille, aperçus, archives, un terminal à côté des fichiers. Il fait partie de
Project Colony : il partage donc l'apparence, la disposition des réglages et le
processus de publication de l'écosystème avec les autres programmes, au lieu
d'inventer les siens.

## Règles persistantes

- **Léger et réactif.** Les dossiers se chargent paresseusement, une page à la
  fois ; les listes et l'arborescence ne rendent que les lignes visibles ; les
  métadonnées sont lues par lots ; chaque cache (miniatures, dossiers, index de
  recherche) est borné en taille et en durée de vie ; le travail bloquant ne
  tourne jamais dans la boucle de rendu.
- **Séparation par responsabilité.** La couche d'accès au système de fichiers
  ignore tout de l'interface, ce qui rend possibles la virtualisation et la
  plupart des tests sans fenêtre. Là où le code enfreint cette règle, l'écart
  est consigné dans
  [architecture.md](../../../../../docs/internals/architecture.md#where-the-code-falls-short)
  (en anglais).
- **La configuration plutôt que le hardcode.** Un comportement qu'un
  utilisateur pourrait raisonnablement vouloir autre est un réglage, versionné,
  migré et validé
  ([configuration.md](../../../../../docs/guide/configuration.md), en anglais).
- **Rust à jour, dépendances à jour.** Rust édition 2024, avec `Cargo.toml` et
  `Cargo.lock` maintenus à jour.
- **Clavier et souris à égalité.** L'interface entière doit pouvoir s'utiliser
  avec l'un comme avec l'autre.
- **Mesuré, pas supposé.** Les affirmations de performance dans le code
  s'accompagnent de la mesure qui les étaye ; les mesures sont dans
  `tests/benches.rs`.

## Décisions techniques

| Décision | Pourquoi |
|---|---|
| **iced 0.14 sur wgpu** pour l'interface | une interface native, dessinée par le GPU, en Rust : la boîte à outils de tous les programmes Colony |
| **colony-ui** pour les couleurs, les polices, les tailles de texte, les animations, les Préférences et les chemins | Xion ressemble au reste de l'écosystème et se comporte comme lui, et un thème ajouté au catalogue partagé y apparaît sans recompiler Xion |
| **Le thème est une sélection dans un catalogue** (famille, variante, contraste élevé, accent), pas un ensemble fermé de noms | un thème inconnu ou retiré dégrade vers la palette par défaut au lieu d'empêcher Xion de démarrer |
| **Fichiers sous `Colony/Xion/`** | la disposition de l'écosystème : les réglages dans le dossier de configuration, la session dans le dossier de données |
| **Interface en anglais avec une traduction française**, qui suit la langue de la session sauf choix contraire | l'anglais est la langue de base de l'écosystème ; le français est la seconde locale que livre chaque programme Colony |
| **`iced_term` (alacritty_terminal) pour le panneau terminal** | un vrai terminal : programmes plein écran, couleurs, Ctrl+C et redimensionnement fonctionnent, là où un ancien modèle maison, un historique de lignes, ne pouvait lancer ni `vim` ni `htop` |
| **`notify` pour l'observateur de fichiers** | le mécanisme natif de chaque plateforme, avec un repli inerte quand il ne peut pas démarrer |
| **Cibles : `x86_64-unknown-linux-gnu` et `x86_64-pc-windows-msvc`** | les deux plateformes que la CI compile et teste, et les deux fichiers publiés à chaque version |
| **`asInvoker` sous Windows**, entrées du menu contextuel sous `HKEY_CURRENT_USER` uniquement | un explorateur de fichiers n'a aucune raison de s'élever, et tout ce qu'il lance hériterait de l'élévation |
| **La suppression passe par la corbeille** par défaut | une suppression définitive est une action à part, derrière une confirmation |
| **Versions signées en CI uniquement**, via le workflow partagé de Project Colony | la clé de signature n'atteint jamais une machine de développement ni un job de compilation ; voir la [politique de signature du code](../../README.md#politique-de-signature-du-code) du README |

## Roadmap documentaire

Points ouverts, repris de la roadmap documentaire de l'ancien `docs/docs.md` en
français. L'architecture des modules, les stratégies de cache, l'indexation et
la recherche, et le modèle de navigation et de sélection sont écrits ; ceux-ci
ne le sont pas :

- [ ] Diagrammes d'interaction : les modules et les flux de données entre eux.
  [architecture.md](../../../../../docs/internals/architecture.md#data-flow)
  en donne une courte esquisse, pas un diagramme.
- [ ] Cycle de rendu Iced et modèle d'état détaillé.
- [ ] Rattacher les exemples de code de `docs/` au crate, pour que
  `cargo test --doc` les compile et qu'ils ne puissent plus pourrir.
- [ ] Porter en tête de chaque page la date à laquelle elle a été confrontée
  au code (voir [Sur la relecture des documents](../README.md#sur-la-relecture-des-documents)).
