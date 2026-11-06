# Travail différé

- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-1-squelette-du-binaire-base-sqlite-et-sante.md`
  summary: Écrire `deploy/.env.example` avec toutes les variables lues par `Config` (dont `FORM_TOKEN_SECRET` ≥ 32 octets).
  evidence: `Config` lit déjà les variables de l'epic 1 ; `stories.yaml` place `deploy/.env.example` en story 1-3.
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-1-squelette-du-binaire-base-sqlite-et-sante.md`
  summary: Tester le binaire au niveau processus (`CARGO_BIN_EXE_server`) : argument inconnu → 2, `healthcheck` sans serveur → ≠ 0, config absente → ≠ 0 avec le nom de la variable sur stderr.
  evidence: seules les fonctions de bibliothèque sont testées ; le consommateur (healthcheck du Dockerfile) arrive en 1-3.
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-1-squelette-du-binaire-base-sqlite-et-sante.md`
  summary: Rendre `embedded_web_dist` strict en CI (variable du type `REQUIRE_WEB_DIST=1` après `npm run build`).
  evidence: sans build web le test se replie sur le contrat 404 ; un `outDir` cassé passerait en vert.
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-2-reference-du-jeu-en-toml-rechargee-a-chaud.md`
  summary: Tester le câblage de `run()` (même `ReferenceStore` pour `AppState` et la tâche, poll initial, tâche lancée), par extraction d'un `run_with(dir, db, listener)` ou par un smoke test du conteneur.
  evidence: aucun test n'exécute `serve` ; passer un `ReferenceStore::new()` neuf à `AppState` laisserait la production en 503 sans casser la suite.
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-3-conteneur-durci-et-ci-complete.md`
  summary: Élargir le balayage des fuites de `check-hardening.sh` (IPv6, TLD hors liste comme `.ovh`, et dossiers hors `deploy/` et `.github/workflows/`).
  evidence: `fd00::` et un hôte en `.ovh` passent (reproduit) ; le périmètre actuel est celui d'AD-10, le reste du dépôt public n'est protégé que par le grep manuel avant commit.
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-3-conteneur-durci-et-ci-complete.md`
  summary: Décider comment vérifier les PR Dependabot (workflow de CI sur PR hébergé par GitHub, sans secret ni runner du NAS, ou relecture manuelle).
  evidence: `deploy.yml` ne tourne que sur `push` et `workflow_dispatch` ; un bump n'est testé qu'après fusion sur `main` (le futur `deploy` reste derrière `needs: ci`).
- source_spec: `_bmad-output/specs/spec-epic-1-socle/stories/1-3-conteneur-durci-et-ci-complete.md`
  summary: En 1-4, consigner et vérifier les versions minimales du NAS (Compose ≥ 2.24 pour `env_file.required`, Engine ≥ 25 pour `healthcheck.start_interval`) et poser la garde côté runner contre les workflows de fork (approbation obligatoire, runner limité au dépôt).
  evidence: un workflow ajouté par une PR de fork s'exécute avant que `check-hardening.sh` ne le voie ; AD-10 prévoit déjà l'approbation des contributeurs externes.
