# Documentation Xion

## But du projet

Créer un explorateur de fichiers en Rust inspiré de Windows File Explorer, Files et Dolphin.

## Règles persistantes

- Rust édition 2024 minimum.
- Toutes les dépendances, `Cargo.toml` et `Cargo.lock` doivent rester à la dernière mise à jour.
- Le projet doit rester léger (RAM/CPU) et très réactif.
- Architecture modulaire : séparer au maximum les modules/fichiers par domaines et responsabilités.
- Éviter le hardcode : préférer configuration, options et extensions.

## Décisions techniques (à maintenir)

- **Pile UI** : Iced + winit + wgpu.
- **Cible UX** : Windows 11 comme référence principale, compatibilité Windows 10 souhaitée.
- **Style UI** : interface native moderne, navigation clavier/souris complète.

## Principes d'architecture

- **Séparation stricte des domaines** :
  - `core/` : types partagés, erreurs, traits, utilitaires.
  - `filesystem/` : accès FS, watch, métadonnées, indexation.
  - `services/` : recherche, thumbnails, historiques, favoris.
  - `ui/` : composants Iced, état, routing, rendering.
- **Modules indépendants** : chaque module doit exposer une API claire et testable.
- **Pas de hardcode** : préférer la configuration, la découverte et les options runtime.

## Principes de performance

- **Chargement paresseux** des dossiers et métadonnées.
- **Virtualisation** des listes et arbres pour les grands volumes.
- **Cache ciblé** pour thumbnails et métadonnées fréquemment utilisées.
- **Batching** des opérations filesystem pour limiter les appels OS.

## Conventions de configuration

- Fichiers de config versionnés et documentés (format à définir).
- Valeurs par défaut minimales, extensibles via options utilisateur.

## Roadmap documentaire

- [ ] Détailler l'architecture des modules (diagrammes + API principales).
- [ ] Décrire le cycle de rendu Iced et le modèle d'état.
- [ ] Documenter les stratégies de cache et d'indexation.

## Documentation à maintenir

- Conventions d'architecture et de modules.
- Principes de performance et de réactivité.
- Décisions techniques importantes.

## Références internes

- `docs/architecture.md` : architecture modulaire et responsabilités des domaines.
- `docs/ui-routing.md` : structure de routing UI et messages de navigation.
