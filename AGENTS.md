# aion2-raid-planner — instructions pour les agents

<!-- bmad:context -->
<!-- Brouillon du 2026-11-05, écrit à la main au kickoff, dépôt sans code. À régénérer avec
     /bmad-project-context dès que la stack existe : toute édition dans ce bloc sera alors remplacée.
     Ce qui doit survivre se place hors des marqueurs. -->

## aion2-raid-planner

Outil web public, sans compte, pour qu'une dizaine de joueurs d'Aion 2 (deux factions, tous les serveurs
d'une région) se trouvent, comparent leurs disponibilités et fixent la date d'un raid (Sanctuaire) ou
d'une expédition. Hébergé sur le NAS personnel de Vincent. L'intention produit vit dans
`docs/intent-dump.md`, la connaissance du jeu dans `docs/domaine-aion2.md`, les artefacts BMAD dans
`_bmad-output/`.

## Politique

- Ne rien pousser sans feu vert explicite de Vincent. Les commits locaux ne demandent pas d'accord.
- Dépôt public : aucun secret, aucune adresse IP, aucun nom d'hôte du NAS, aucune donnée de joueur réel
  dans le code, les docs ou les commits.
- Sécurité non négociable (`docs/intent-dump.md` §4) : conteneurs non-root ; jamais `privileged`,
  `pid: host`, socket Docker ni montage de répertoires du NAS ; secrets uniquement par `.env` non versionné.
- Pas de login, pas de mot de passe, pas d'email. Une story qui en aurait besoin s'arrête et remonte à
  Vincent : c'est un changement de produit, pas un détail technique.
- Les décisions produit se prennent dans les documents BMAD (brief, spine, SPEC) avant d'être codées. Un
  document = un writer : ne pas éditer `SPEC.md`, la spine ni `sprint-status.yaml` à la main.

## Conventions

- Prose en français ; identifiants, chemins, noms de skills, statuts et clés YAML/TOML en anglais tels quels.
- Dates et heures stockées en UTC, affichées dans le fuseau du navigateur.
- Données de référence du jeu (régions, serveurs, factions, instances, classes, rôles) dans des fichiers de
  données éditables, jamais en dur dans le code ni dans l'interface.

## Pointeurs

- Intention : `docs/intent-dump.md` (questions ouvertes Q1 à Q11 au §10). Domaine : `docs/domaine-aion2.md`.
- Artefacts : `_bmad-output/planning-artifacts/`, `_bmad-output/specs/`, `_bmad-output/implementation-artifacts/`.
- Overrides BMAD d'équipe : `_bmad/custom/` (committés) ; les `*.user.toml` restent locaux.
<!-- /bmad:context -->
