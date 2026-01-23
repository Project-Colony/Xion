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

## État d'avancement (vérification)

- [x] Base Rust 2024 et dépendances cœur (Iced / winit / wgpu) en place.
- [x] Architecture modulaire (core / filesystem / services / ui) intégrée.
- [x] UI Iced fonctionnelle (tabs, breadcrumbs, raccourcis, menu contextuel).
- [x] Virtualisation des listes + caches métadonnées/thumbnails.
- [x] Configuration TOML versionnée + migration + reload.
- [x] Polices Nerd Fonts intégrées et chargées.
- [ ] Opérations de fichiers complètes (copier/coller, renommer, supprimer) branchées au backend.
- [ ] Navigation “Explorer-like” (arbre latéral complet, lecteurs/volumes, Quick Access).
- [ ] Vues détaillées configurables (colonnes, tri avancé, regroupement).
- [ ] Recherche indexée et filtres avancés (type, date, taille).

## Prochaines priorités (cap Explorer)

- [ ] Implémenter les actions de fichiers réelles (copie, déplacement, suppression, renommage).
- [ ] Ajouter l’arbre latéral (lecteurs, favoris, dossiers épinglés).
- [ ] Mettre en place une barre d’adresse éditable + historique.
- [ ] Construire la vue “Détails” (colonnes configurables, tailles, tri).
- [ ] Préparer la recherche avec indexation asynchrone.
