---
title: 'Déploiement par runner, edge et sauvegardes'
type: 'feature'
created: '2026-11-06'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '20133575f584a4b038b00df9869ebc03f25a024c'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** L'image et la CI existent, mais rien ne livre sur le NAS : pas de job `deploy`, pas de sauvegarde, pas de restauration connue, pas de configuration d'edge ; le site n'est pas en ligne.

**Approach:** Job `deploy` sur le runner du NAS piloté par un script de plan testé, sous-commandes `backup` et `restore` du binaire (l'image `scratch` n'a pas de `sqlite3`), tâche de sauvegarde quotidienne en processus, exemples Caddy VPS et NAS, et `deploy/README.md` pas à pas pour les gestes de Vincent jusqu'au premier déploiement (SPEC CAP-5, CAP-6 ; AD-2, AD-9, AD-10, AD-14).

## Boundaries & Constraints

**Always:** le clone du NAS est remis exactement sur `$GITHUB_SHA` (le commit testé par `ci`), pas sur un `origin/main` qui aurait avancé ; la base de comparaison est la révision de l'image en place (label OCI `org.opencontainers.image.revision` = `APP_BUILD`), pas le HEAD du clone ; le garde-fou de migrations passe avant toute construction ; `reference/` est toujours copié par `docker compose cp` ; le health-check vérifie 200 et, après reconstruction, `X-App-Build` = `$GITHUB_SHA` ; sauvegardes écrites dans un fichier temporaire puis renommées ; chaque sauvegarde vieillit au plus de deux jours (AD-14) ; placeholders `example.org` et noms en capitales (`VPS_TUNNEL_IP`) dans `deploy/caddy/` et le README.

**Never:** aucune IP, aucun nom d'hôte ni chemin réel du NAS dans le dépôt ; aucun `actions/checkout` ni aucun secret GitHub dans le job `deploy` ; pas de conteneur auxiliaire, de bind mount ni de `sqlite3` de l'hôte pour sauvegarder ou restaurer ; pas de cron du NAS (AD-14) ; aucun geste sur le NAS, le VPS ou le DNS par l'agent ; pas de sauvegarde hors NAS, de staging ni de purge.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Premier déploiement | aucune image locale | build complet, pas de garde-fou ni de sauvegarde préalable | `::warning::` dans le journal |
| Code modifié | base..HEAD touche hors `reference/` | `up -d --build`, `cp reference/`, health + `X-App-Build` | job rouge, journaux du conteneur |
| Référence seule | base..HEAD ne touche que `reference/` | pas de build, `cp reference/`, health | idem |
| Migration ajoutée | ligne `A` dans `crates/server/migrations/` | `backup pre-deploy` dans le conteneur en place, puis build | échec de la sauvegarde → rouge avant `up` |
| Migration modifiée ou supprimée | ligne `M`, `D` ou `R` | job rouge, fichier nommé, conteneur intact | N/A |
| Base inconnue | révision de l'image absente du clone (historique réécrit) | job rouge avant tout `up` | procédure du README |
| Sauvegarde quotidienne | tick horaire, dernière `daily-*` de plus de 24 h ou absente | `VACUUM INTO` → `daily-<ts>.db`, deux `daily` gardées, `pre-deploy` de plus de 2 jours supprimées | erreur journalisée, nouvel essai au tick suivant |
| `restore <nom>` | service arrêté, `docker compose run --rm server restore <nom>` | `integrity_check` du fichier, suppression `-wal`/`-shm`, copie atomique sur `app.db` | nom inconnu ou fichier corrompu → ≠ 0, `app.db` intact |
| `restore` sans argument | — | liste des sauvegardes, code 2 | N/A |

**Décisions (Vincent, 2026-11-06) :**
- Runner sous un utilisateur dédié `aion2-deploy`, membre du groupe `docker`, runner de dépôt dans son home ; le README le fait créer.
- Spec gardée entière malgré ~2 700 tokens.

</frozen-after-approval>

## Code Map

- `.github/workflows/deploy.yml` -- job `ci` existant (1-3) à garder ; ajouter `deploy` et étendre l'étape « Conteneur de fumée ».
- `deploy/Dockerfile` -- `ARG APP_BUILD` en l. 16 ; ajouter le `LABEL` de révision dans l'étape `scratch`. `/data/backups` déjà créé.
- `deploy/docker-compose.yml` -- `APP_BUILD: ${APP_BUILD:-dev}` déjà passé ; service `server`, image `aion2-raid-planner:local`, `${PORT:-8080}`. Ne pas toucher au durcissement.
- `deploy/check-hardening.sh` -- balaie `deploy/` et `.github/workflows/` (IP, hôtes hors `example.*`/`github.com`/`localhost`, déclencheurs) : le README et `caddy/` doivent passer.
- `crates/server/src/main.rs` -- dispatch des sous-commandes et message d'usage ; `run()` lance `tasks::reload::spawn`.
- `crates/server/src/app/tasks/{mod,reload}.rs` -- modèle de boucle `interval_at` + `MissedTickBehavior::Delay`.
- `crates/server/src/infra/sqlite.rs` -- `DB_PATH`, `file_options`, `with_pragmas`, `Db`.
- `crates/server/src/infra/clock.rs` -- `Clock`, `FixedClock` pour tester l'échéance quotidienne.
- `crates/server/tests/cli.rs` -- tests processus `CARGO_BIN_EXE_server`, à étendre.
- Motif voisin (mécanisme seulement) : runners existants en service systemd, un par dépôt, `~/actions-runner-<dépôt>` ; Caddy du NAS en service de l'hôte, bloc `http://` partageant `:80`, `@nottunnel not remote_ip` puis `handle { abort }`.
- NAS vérifié le 2026-11-06 : Engine ≥ 25 et Compose ≥ 2.24 satisfaits, Caddy 2 en service de l'hôte ; ni `sqlite3` sur l'hôte.

## Tasks & Acceptance

**Execution:**
- [x] `crates/server/src/infra/backup.rs` -- `vacuum_into(pool, dir, kind, now)`, `prune(dir, now)`, `daily_due(dir, now)`, `restore(dir, name, db_path)`, `list(dir)` ; noms `daily-YYYYMMDDTHHMMSSZ.db` et `pre-deploy-….db` ; tests sur dossier temporaire (`tempfile` en dev-dependency) -- AD-2.
- [x] `crates/server/src/app/tasks/backup.rs`, `main.rs` -- tâche horaire lancée dans `run()` avec `Clock` ; sous-commandes `backup [daily|pre-deploy]` (sans `Config`, sans migration) et `restore [NOM]` ; usage mis à jour ; `tests/cli.rs` étendu -- AD-14.
- [x] `deploy/plan-deploy.sh`, `deploy/tests/plan-deploy.test.sh` -- POSIX `sh`, `plan-deploy.sh BASE HEAD` dans un dépôt git : stdout `rebuild=0|1` et `backup=0|1`, sortie ≠ 0 avec le fichier fautif sur `M`/`D`/`R` de migration ou base inconnue, `BASE` vide = premier déploiement ; tests sur dépôts git temporaires -- matrice.
- [x] `deploy/Dockerfile` -- `LABEL org.opencontainers.image.revision=$APP_BUILD`.
- [x] `.github/workflows/deploy.yml` -- job `deploy` : `needs: ci`, `runs-on: [self-hosted, nas]`, `concurrency: {group: deploy-nas, cancel-in-progress: false}`, `timeout-minutes`, clone `$HOME/aion2-raid-planner` : `fetch` + `reset --hard $GITHUB_SHA`, base lue sur l'image, `plan-deploy.sh`, `exec -T server /server backup pre-deploy` si demandé, `up -d --build` avec `APP_BUILD=$GITHUB_SHA` si demandé, `cp reference/.`, dix essais de health ; job `ci` : test de `plan-deploy.sh`, et fumée étendue (`cp reference/.`, `backup pre-deploy`, `stop`, `run --rm server restore <nom>`, `up --wait` healthy).
- [x] `deploy/caddy/vps.Caddyfile`, `deploy/caddy/nas.caddy` -- VPS : TLS, `encode`, HSTS, `-Server`, `header_up X-Client-IP {remote_host}` (écrase), relais de toutes les méthodes vers `VPS_TUNNEL_IP` côté NAS, aucun `log` ; NAS : `http://raid.example.org`, `@nottunnel` + `abort` en premier, `reverse_proxy localhost:8080`, aucun `log` ; un commentaire par ligne de garde -- AD-9.
- [x] `deploy/README.md` -- pas à pas : prérequis vérifiés, réglages GitHub (runner de dépôt, approbation des workflows externes), installation du runner, clone et `.env` (génération du secret), premier déploiement par `workflow_dispatch`, blocs Caddy NAS puis VPS, DNS sans enregistrement vers le domicile, vérifications (en-têtes d'AD-9, `/api/health`), sauvegardes, restauration pas à pas, base inconnue, retour arrière (revert, sauf migration).

**Acceptance Criteria:**
- Given le dépôt, when les commandes du job `ci` tournent en local, then toutes passent, dont `plan-deploy.test.sh` et `check-hardening.sh` sur le nouveau contenu de `deploy/`.
- Given le compose lancé en local avec un `.env` de test, when on joue `cp reference/.`, `backup pre-deploy`, puis `stop`, `restore` et `up`, then le conteneur redevient `healthy` et les données sauvegardées sont présentes.
- Given le premier déploiement fait par Vincent, when il ouvre l'URL publique et joue une restauration, then `/api/health` est vert, les en-têtes d'AD-9 sont présents et la restauration a réussi (done_checkpoint).

## Design Notes

Base = révision de l'image plutôt que HEAD du clone : un push refusé par le garde-fou laisse le HEAD avancé ; le push suivant comparerait alors depuis le commit refusé et laisserait passer sa migration modifiée. L'image ne change que si le build a eu lieu.

`restore` passe par `docker compose run --rm server` : même image, même utilisateur, même durcissement, service arrêté ; c'est la seule façon de supprimer `-wal`/`-shm` dans un volume sans shell. Ordre : contrôle d'intégrité, suppression `-wal`/`-shm`, copie vers `app.db.tmp`, `fsync`, `rename`.

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && REQUIRE_WEB_DIST=1 cargo test` (après `npm run build`) -- expected: vert.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh && sh deploy/tests/plan-deploy.test.sh` -- expected: 0.
- Fumée locale du compose avec un `.env` de test hors dépôt (scénario de l'AC 2), puis `down -v` -- expected: `healthy` après restauration.

## Implementation Notes

- `restore` : ordre légèrement durci par rapport aux Design Notes — contrôle d'intégrité (en-tête SQLite exigé, puis `integrity_check` en `immutable`), copie vers `app.db.tmp` + `fsync`, puis seulement suppression de `-wal`/`-shm`, `rename` et `fsync` du dossier. Un disque plein pendant la copie laisse `app.db` et son WAL intacts. L'en-tête est exigé car SQLite ouvre un fichier trop court comme une base vide valide.
- `backup` sans argument = `pre-deploy` (une sauvegarde manuelle n'évince pas une `daily`). La base doit exister (`create_if_missing(false)`). Le nom écrit sort seul sur stdout.
- `prune` supprime aussi les `*.db.tmp` abandonnés de plus de deux jours, jamais un `.tmp` récent qu'un autre processus écrirait.
- `plan-deploy.sh` : seuls les `*.sql` de `crates/server/migrations/` comptent (supprimer `.gitkeep` n'est pas une faute) ; BASE doit être un SHA complet présent dans le clone et ancêtre de HEAD, sinon « unknown base ». Une image sans label est passée comme `missing`, donc refusée.
- Job `deploy` : `if: github.ref == 'refs/heads/main'` (un `workflow_dispatch` peut viser une autre branche), `permissions: {}`, `COMPOSE_FILE` en variable de job. Le `::warning::` du premier déploiement est émis par le workflow.
- Caddy : upstream du VPS nommé `NAS_TUNNEL_IP` (adresse du NAS dans le tunnel), matcher du NAS sur `VPS_TUNNEL_IP` ; les deux fichiers passent `caddy validate` et `caddy fmt` (2.10) avec des adresses de test.
- AC 2 : sans migration ni route d'écriture à ce stade, la fumée prouve le chemin conteneur (sauvegarde, liste, restauration, `healthy`) ; la présence des données restaurées est prouvée par `infra::backup::tests::vacuum_into_then_restore_brings_data_back`.

## Spec Change Log

## Review Triage Log

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | verif | branchement de la tâche quotidienne dans `serve` jamais vérifié | medium | tests unitaires sur `tick()` seulement ; la fumée ne cherche que le nom `pre-deploy` | patch |
| 2 | verif | lien sortie de `plan-deploy.sh` ↔ `if:` du job `deploy` jamais exécuté en CI | medium | le job ne tourne que sur le runner du NAS | defer |
| 3 | verif, edge | `backup_now` sort en 1 sans nom si la rotation échoue après une sauvegarde écrite | low | `prune(...)?` après `vacuum_into` ; correction directe | patch |
| 4 | blind | base lue sur le tag de l'image, pas sur le conteneur en service | medium | build réussi puis `up` en échec : le tag avance, la migration suivante passe sans `pre-deploy` | patch |
| 5 | blind | image supprimée = premier déploiement malgré une base existante | low | une image utilisée par un conteneur, même arrêté, n'est pas supprimée par `prune` ; cas manuel rare | rejeté |
| 6 | blind, edge | run ancien terminé après un plus récent → `unknown base` rouge, README trompeur | medium | `concurrency` sérialise sans annuler ; HEAD ancêtre de la base | patch |
| 7 | blind | push sans reconstruction ne relance pas un conteneur arrêté | low | `cp` passe sur un conteneur arrêté, la santé échoue ; correction directe | patch |
| 8 | blind, edge | `restore` ne vérifie pas que le service est arrêté | low | précondition de la matrice et du README ; un verrou exclusif ne détecte pas un serveur WAL inactif, et `run --rm` n'atteint pas le réseau du service | rejeté |
| 9 | blind | restauration non réversible (pas de `pre-restore`) | low | nouveau type de sauvegarde, hors intention ; opérateur unique guidé par la liste | rejeté |
| 10 | blind | échecs de sauvegarde visibles seulement dans les journaux | medium | aucune exposition de l'âge de la dernière `daily` ; nouvelle surface (santé ou job) | defer |
| 11 | blind | rétention : `pre-deploy` supprimée à 2 jours, non dit au §13 | low | AD-14 fixe la borne ; phrase manquante au README | patch |
| 12 | blind | la fumée ne prouve pas le retour des données | low | aucune route d'écriture ni migration à ce stade ; prouvé par `vacuum_into_then_restore_brings_data_back` | rejeté |
| 13 | blind, edge | clone non nettoyé après `reset --hard` | low | fichiers non suivis dans le contexte de build ; une ligne | patch |
| 14 | blind | chemins fixes, tests CLI limités aux codes de sortie | low | comportement fichier couvert dans `infra::backup` ; surcharge = nouvelle surface | rejeté |
| 15 | blind, edge | `plan-deploy.sh` : branche `C` morte, chemins cités par git, renommage vers `migrations/` | low | migrations nommées `NNNN_nom.sql` par le projet ; refus conservateur du renommage | rejeté |
| 16 | blind | versions relevées du NAS publiées dans le README | low | empreinte du NAS dans un dépôt public ; suppression directe | patch |
| 17 | blind | `reverse_proxy localhost:8080` alors que le port est publié sur `127.0.0.1` | low | `::1` possible ; correction directe | patch |
| 18 | edge | fichier de `reference/` supprimé qui reste dans le volume | false | cinq fichiers fixes (AD-6), retrait par `enabled = false` ; un fichier manquant fait échouer `validate-reference` en CI | rejeté |
| 19 | edge | `backup pre-deploy` en échec si le conteneur est arrêté | low | job rouge explicite, cas rare ; la matrice demande le conteneur en place | rejeté |
| 20 | edge | crash entre suppression du `-wal` et renommage | low | la base remplacée est celle qu'on restaure ; relancer `restore` suffit | rejeté |
| 21 | edge | `daily` horodatée dans le futur (horloge corrigée) bloque les suivantes | low | horloge du NAS synchronisée ; cas rare, garde supplémentaire | rejeté |
