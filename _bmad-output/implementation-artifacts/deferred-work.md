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
