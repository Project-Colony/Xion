<!-- xion-i18n: source=docs/README.md sha=01e6906812ff20bb51c9109e44f7ed34dff2086e -->

# Documentation de Xion

Triée par lecteur, pas par sujet. Seules cette page et « Objectifs et
décisions » sont traduites ; les autres pages sont en anglais.

## Utiliser Xion

| | |
|---|---|
| [guide/configuration.md](../../../../docs/guide/configuration.md) | où vivent les réglages, chaque clé de `config.toml`, et les raccourcis clavier |

## Lire le code

| | |
|---|---|
| [internals/architecture.md](../../../../docs/internals/architecture.md) | comment le crate est découpé, compilé et testé, et où le code s'écarte de ses propres règles |
| [internals/ui-routing.md](../../../../docs/internals/ui-routing.md) | ce que sont une route, une sélection et un onglet, et comment la navigation passe de l'un à l'autre |
| [internals/search.md](../../../../docs/internals/search.md) | comment l'index de recherche est construit, sur quoi il filtre, et comment l'interface le pilote |

## Pourquoi il existe

| | |
|---|---|
| [project/decisions.md](project/decisions.md) | à quoi sert Xion, les choix techniques qui en découlent, et la documentation qui reste à écrire |

## Sur la relecture des documents

Aucune page ne porte de case « relu » à cocher. Une case cochée ne prouve pas
qu'une relecture a eu lieu : un ancien index certifiait que trois documents
étaient revus et alignés sur l'implémentation, alors que deux d'entre eux
décrivaient des structures et des chemins qui n'existaient plus.

À la place, chaque page cite ses sources : un fichier et un symbole, ou un nom
de test. Un numéro de ligne est un relevé daté ; en cas de divergence avec le
code, c'est le symbole cité qui fait foi.

Politique de sécurité : [SECURITY.md](../../../../SECURITY.md).
