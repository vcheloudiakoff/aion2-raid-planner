---
title: 'Conteneur durci et CI complète'
type: 'feature'
created: '2026-11-06'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '993401f3e164c99a1e8d48354f09c4ed68f73d47'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
  - '{project-root}/_bmad-output/specs/spec-epic-1-socle/configuration.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Le binaire n'a ni image ni compose : rien ne garantit qu'il tournera sur le NAS sans prise sur l'hôte, et aucun push n'est vérifié avant d'y arriver.

**Approach:** Dockerfile multi-étapes `node → rust (musl) → scratch`, `deploy/docker-compose.yml` conforme à AD-8, `deploy/.env.example`, `deploy/check-hardening.sh` prouvé par fixtures, et `.github/workflows/deploy.yml` avec le seul job `ci` qui exécute la liste d'AD-10 (SPEC CAP-4, AD-8, AD-10, AD-17, AD-19).

## Boundaries & Constraints

**Always:** chaque ligne de durcissement du compose porte un commentaire d'une ligne qui dit ce qu'elle empêche ; image finale `FROM scratch`, binaire statique, `USER 10001:10001`, `/data` (avec `reference/` et `backups/`) copié `--chown=10001:10001` ; service nommé `server` ; healthcheck = `/server healthcheck` ; `APP_BUILD` passé en argument de build ; versions exactes de la table Stack (images comprises) ; actions GitHub épinglées par SHA complet avec la version en commentaire ; `permissions: contents: read` ; déclencheurs `push` sur `main` et `workflow_dispatch` seulement ; `.dockerignore` qui exclut `.env`, `target/`, `node_modules/`, `.git/`.

**Never:** aucune adresse IP ni nom d'hôte réel (placeholders `example.com`) ; pas de job `deploy`, de runner auto-hébergé, d'edge ni de sauvegarde (1-4) ; pas de `list-error-codes` (ajouté en 1-5 avec la sous-commande) ; aucune valeur secrète dans `.env.example` ; aucun bind mount, même pour `reference/`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Compose livré | `deploy/docker-compose.yml` | `check-hardening.sh` → 0 | N/A |
| Motif interdit | fixture avec `privileged`, `pid:`, `docker.sock`, `cap_add`, `network_mode: host`, bind mount | → ≠ 0, motif nommé sur stderr | un fixture par motif |
| Clé manquante | fixture sans `user`, `read_only`, `cap_drop`, `mem_limit`, `cpus`, `no-new-privileges`, `restart`, `logging`, ou port sans `127.0.0.1:` | → ≠ 0, clé nommée | un fixture par clé |
| Fuite | IPv4 hors `127.0.0.1`/`0.0.0.0`, ou nom d'hôte hors liste blanche, dans `deploy/` ou `.github/workflows/` | → ≠ 0, fichier et ligne | fixtures exclus du balayage ; IP de test assemblée par le script de test, jamais littérale |
| Workflow PR | un workflow déclenché par `pull_request` ou `pull_request_target` | → ≠ 0 | protège le futur runner du NAS |
| Crate réseau | `reqwest`, `ureq`, `lettre`, `hickory`, ou `hyper-util feature "client…"` dans `cargo tree -e features` | job `ci` rouge | N/A |
| Budget | JS initial (entrée + `modulepreload` de `index.html`) > 150 ko gzip, ou CSS > 20 ko gzip | job `ci` rouge, tailles affichées | N/A |
| Image | `docker compose build` puis `up` avec un `.env` de test | conteneur `healthy`, processus en `10001:10001` | N/A |
| Fumée en CI | push sur `main` | job `ci` construit l'image, `up` avec un `.env` de test généré, attend `healthy`, vérifie l'utilisateur `10001:10001` et `GET /api/reference` 200, puis `down -v` | job rouge, journaux du conteneur affichés |
| Binaire | argument inconnu / `healthcheck` sans serveur / `serve` sans config | code 2 / ≠ 0 / ≠ 0 avec le nom de la variable sur stderr | test `CARGO_BIN_EXE_server` (reprise de `deferred-work.md`) |

**Décisions (Vincent, 2026-11-06) :**
- Test de fumée du conteneur dans le job `ci` (+5 à 10 min par push), pour arrêter avant le NAS un Dockerfile cassé, un durcissement bloquant ou une référence mal câblée.
- Spec gardée entière malgré ~2 400 tokens.

</frozen-after-approval>

## Code Map

- `crates/server/src/main.rs` -- sous-commandes `serve`, `healthcheck`, `validate-reference [DIR]` ; écoute `0.0.0.0:8080` ; chemins fixes `/data/app.db`, `/data/reference`.
- `crates/server/build.rs` -- lit `APP_BUILD` (≤ 64 car. `[A-Za-z0-9._-]`), `dev` sinon ; n'invoque jamais npm.
- `crates/server/src/infra/reference.rs` -- `include_str!("../../../../reference/*.toml")` : le contexte Docker doit contenir `reference/`.
- `crates/server/src/config.rs` -- variables lues ; valeur vide = absente, donc `${VAR:-}` est sûr dans le compose.
- `crates/server/tests/http.rs::embedded_web_dist` -- se replie sur 404 sans build web : à rendre strict sous `REQUIRE_WEB_DIST=1` (entrée de `deferred-work.md`).
- `web/package.json` -- scripts `build` (`tsc --noEmit && vite build`), `typecheck` ; pas de `vitest` ni de `package-lock` à jour pour lui.
- `.gitignore` -- ignore `.env`, `.env.*` sauf `.env.example`.
- Outils locaux : Docker 29.6 et Compose v5.2 présents ; ni `shellcheck`, ni `actionlint`, ni `cargo-deny`.

## Tasks & Acceptance

**Execution:**
- [x] `deploy/Dockerfile`, `.dockerignore` -- étapes `node:24.22.2-alpine` (`npm ci`, `npm run build`), `rust:1.99.1-alpine` (`musl-dev`, caches BuildKit, `ARG APP_BUILD`, `cargo build --release --locked -p server`), `scratch` -- AD-1, AD-8.
- [x] `deploy/docker-compose.yml`, `deploy/.env.example` -- AD-8 complet, `env_file: .env` plus obligatoires en `${VAR:?}`, port `127.0.0.1:${PORT:-8080}:8080`, `tmpfs /tmp` `noexec,nosuid,nodev`, healthcheck ; `.env.example` avec toutes les variables d'epic 1 de `configuration.md` et la commande pour générer le secret -- AD-8.
- [x] `deploy/check-hardening.sh`, `deploy/tests/check-hardening.test.sh`, `deploy/tests/fixtures/` -- POSIX `sh` sans `yq`, arguments optionnels (compose, dossiers), une ligne par violation -- matrice.
- [x] `web/scripts/check-budget.mjs` (+ test vitest), `web/package.json` -- `vitest` 5.1.0 en devDependency, script `test` -- AD-19.
- [x] `crates/server/tests/http.rs`, `crates/server/tests/cli.rs` -- `REQUIRE_WEB_DIST=1` rend `embedded_web_dist` strict ; codes de sortie du binaire.
- [x] `.github/workflows/deploy.yml` -- job `ci` sur `ubuntu-latest` : `npm ci`, `tsc`, `vitest`, `npm audit --audit-level=high`, `vite build` + budget, puis `cargo fmt --check`, `clippy -D warnings`, `cargo test` avec `REQUIRE_WEB_DIST=1`, contrôle `cargo tree`, `cargo deny check advisories` (0.20.2), `validate-reference reference`, `check-hardening.sh` et son test, puis test de fumée du conteneur (cache BuildKit des couches).
- [x] `.github/dependabot.yml` -- hebdomadaire : `cargo`, `npm` (`/web`), `github-actions`, `docker` (`/deploy`).

**Acceptance Criteria:**
- Given le dépôt, when chaque commande du job `ci` est lancée en local (sauf `cargo deny` si absent), then toutes passent.
- Given `deploy/docker-compose.yml`, when `docker compose config` avec un `.env` de test, then il est valide ; sans `PUBLIC_BASE_URL`, il échoue en nommant la variable.
- Given l'image construite, when on l'inspecte, then elle ne contient que `/server` et `/data`, et `Config.User` vaut `10001:10001`.

## Design Notes

Liste blanche de noms d'hôte, dans le script : `example.com`, `example.net`, `example.org`, `*.example`, `localhost`, plus les domaines techniques indispensables au workflow s'il en faut (`github.com`). Détection : jetons `[a-z0-9-]+(\.[a-z0-9-]+)*\.(tld)` sur une liste de TLD publics courants et de suffixes de réseau privé (`lan`, `local`, `home`, `internal`, `arpa`, `ts.net`), pour éviter les faux positifs sur `app.db` ou `docker-compose.yml`.

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && (cd web && npm ci && npm run build && npx vitest run && node scripts/check-budget.mjs) && REQUIRE_WEB_DIST=1 cargo test` -- expected: vert.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh` -- expected: 0.
- `docker compose -f deploy/docker-compose.yml --env-file <.env de test hors dépôt> up -d --build`, puis `docker inspect` -- expected: `healthy`, utilisateur `10001:10001` ; `down -v` ensuite.

## Implementation Notes

- `rust-toolchain.toml` n'est pas copié dans l'image : `rust:1.99.1-alpine` fournit déjà la version, sans télécharger clippy ni rustfmt.
- `env_file` en `required: false` pour que le test de fumée passe par `--env-file` ; les deux obligatoires restent en `${VAR:?}`.
- À vérifier sur le NAS en 1-4 : Compose ≥ 2.24 (`env_file.required`), Engine ≥ 25 (`healthcheck.start_interval`).
- `cargo-deny` 0.20.2 téléchargé depuis sa release officielle et vérifié par SHA-256 dans le workflow.
- Contrôle AD-17 sorti dans `deploy/check-network-crates.sh` (stdin, testé par lignes types) ; `check-hardening.sh` vérifie les clés par service, refuse les échappements vers l'hôte et n'admet que les déclencheurs `push`, `workflow_dispatch`, `schedule`.
- Le balayage des noms d'hôte signale aussi `.env.local` (`local` est un suffixe privé) : nommer autrement les fichiers de test.
- Résout trois entrées de `deferred-work.md` (`.env.example`, codes de sortie du binaire, `REQUIRE_WEB_DIST`) et l'entrée 1-2 sur le câblage de `run()` (fumée : `/api/reference` 200 dans le conteneur).

## Spec Change Log

## Review Triage Log

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | verif, blind, edge | clés obligatoires comptées pour tout le fichier : un 2e service non durci, ou `read_only` dans un volume, passe | medium | `has[]` global, reproduit (exit 0) | patch |
| 2 | blind, edge | `devices`, `volumes_from`, `ipc`/`uts`/`userns_mode` `host`, `unconfined`, `sysctls`, `secrets`/`configs` `file:`, `include`/`extends` non refusés | medium | reproduit (exit 0) ; contournent l'intention « aucune prise sur le NAS » | patch |
| 3 | edge | clé entre guillemets (`"privileged": true`) contourne les motifs | low | motifs sans guillemets ; correction d'expression régulière | patch |
| 4 | edge | `user: "10001:0"` ou `${UID}` accepté | low | seul `0`/`root` en tête est refusé | patch |
| 5 | edge | échec d'`awk` ignoré, sortie 0 | medium | statut d'`awk` non lu, reproduit | patch |
| 6 | blind, edge | déclencheurs vérifiés par liste noire (`pull_request` seul) | medium | `workflow_run`, `issue_comment` passent, reproduit | patch |
| 7 | edge | `trap` sur INT/TERM nettoie sans sortir | low | handler sans `exit` | patch |
| 8 | verif, blind, edge | contrôle `cargo tree` en ligne, non testé, sans `hyper` client, `curl`, `isahc`… | medium | motif seulement dans le YAML ; une faute de frappe le rendrait muet | patch |
| 9 | blind | la fumée ne vérifie pas le durcissement appliqué par Docker | low | seuls santé, utilisateur, `/api/reference` ; assertions `docker inspect` directes | patch |
| 10 | edge | `docker top` sans ligne de processus passe | low | `awk` sans compteur | patch |
| 11 | edge | étape « Journaux » en erreur sans `smoke.env` | low | interpolation `:?` sans le fichier | patch |
| 12 | verif, blind | branches du parseur sans fixture (`driver_opts`, ports longs, `source: ./`, multi-services…) | medium | supprimer les lignes 87-88 ou 101 laisse les 34 tests verts | patch |
| 13 | blind, edge | test `healthcheck` faux en local si un service répond 200 sur 8080 | low | port fixe ; saut si le port est pris | patch |
| 14 | edge | `check-budget.mjs` muet via un lien symbolique | low | comparaison `argv[1]` littérale, reproduit | patch |
| 15 | blind, edge | balayage des fuites limité (IPv6, TLD hors liste, hors `deploy/`/workflows) | medium | reproduit avec `fd00::` et `.ovh` ; périmètre fixé par AD-10 | defer |
| 16 | blind | PR Dependabot fusionnées sans CI | low | `ci` ne tourne qu'après fusion ; le futur `deploy` reste derrière `needs: ci` | defer |
| 17 | blind, edge | versions Docker minimales non consignées (Compose ≥ 2.24, Engine ≥ 25) | medium | `env_file.required`, `start_interval` | defer |
| 18 | blind | un workflow de fork s'exécute avant `check-hardening` | medium | garde côté runner (approbation, groupe) déjà prévue par AD-10 en 1-4 | defer |
| 19 | blind | versions Rust/Node dupliquées Dockerfile / CI | low | dérive possible après un bump Dependabot ; sans effet sur la sûreté | rejeté |
| 20 | blind | `pids_limit: 128` trop bas pour Tokio | false | sqlx-sqlite utilise ses propres fils (5 connexions), `spawn_blocking` seulement au tick | rejeté |
| 21 | blind | pas de `concurrency` | low | `concurrency: deploy-nas` arrive avec le job `deploy` (AD-10, 1-4) | rejeté |
| 22 | blind | `.env.example` sans `APP_BUILD`, vide passé en chaîne vide | false | `APP_BUILD` est un argument de build du job `deploy` ; `Config` traite vide = absent (`config.rs`) | rejeté |
| 23 | blind | pas d'init ni de `STOPSIGNAL` | false | le binaire PID 1 installe son gestionnaire SIGTERM (`main.rs::shutdown_signal`) | rejeté |
| 24 | edge | canal de `rust-toolchain.toml` mal lu → erreur peu claire | low | format fixé dans le dépôt | rejeté |
| 25 | edge | `check-budget` : `base`, query, attribut sans guillemets | low | sortie Vite par défaut, `base` `/` | rejeté |
| 26 | edge | volume existant appartenant à un autre utilisateur | low | volume neuf au premier déploiement ; procédure en 1-4 | rejeté |
| 27 | edge | symlink non balayé | low | aucun lien dans `deploy/` ni les workflows | rejeté |
| 28 | edge | `list-error-codes` absent alors que « liste d'AD-10 » | false | exclu explicitement par la spec (1-5) | rejeté |
