# Xion Explorer

Xion est un explorateur de fichiers Rust inspiré de Windows File Explorer, Files et Dolphin.
Ce dépôt pose les bases du projet ainsi que les règles de développement.

## Objectifs

- Construire un explorateur de fichiers moderne en Rust.
- Prioriser une empreinte **RAM minimale**, une **consommation CPU faible** et une **réactivité maximale**.
- Garder une architecture **modulaire** et **bien structurée** (modules/fichiers séparés par domaines).
- Ne rien hardcoder : configuration et comportements doivent être extensibles et configurables.

## Exigences techniques

- Rust **édition 2024 minimum**.
- Toutes les dépendances, `Cargo.toml` et `Cargo.lock` doivent rester **à jour** en permanence.
- Organisation claire des dossiers, documentation et suivi des tâches obligatoires.

## Stack choisie

- **UI** : Iced
- **Fenêtrage/événements** : winit
- **Rendu** : wgpu

## Inspirations UI/UX

- Windows File Explorer
- Files
- Dolphin

## Structure du dépôt

- `docs/` : documentation, décisions techniques, guides.
- `tasks/` : suivi des tâches, obligations, étapes de livraison.

Consultez `docs/docs.md` et `tasks/tasks.md` pour les détails et les obligations persistantes du projet.
