---
title: 'Squelette du binaire, base SQLite et santé'
type: 'feature'
created: '2026-11-06'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '7421c56888ce8de215e286774f3596ebf2100b8c'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
  - '{project-root}/_bmad-output/specs/spec-epic-1-socle/configuration.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Le dépôt ne contient aucun code ; toutes les stories suivantes ont besoin du binaire unique, de sa base SQLite, de ses types de temps et d'identifiants, et d'un état de santé vérifiable.

**Approach:** Créer le workspace Cargo `domain` + `server` et une SPA Vite minimale embarquée par `rust-embed` ; le binaire lit sa `Config` (fail-fast), ouvre SQLite en deux pools, applique les migrations, sert `/api/health`, la SPA avec repli `index.html`, et pose ses en-têtes de sécurité (SPEC CAP-1, CAP-2, CAP-6 côté application).

## Boundaries & Constraints

**Always:** versions de la table *Stack* de la spine ; couches `http → app → infra`, `domain` sans I/O ; `Clock` et générateur d'ids et de secrets injectés par traits ; secrets 256 bits base64url sans padding (43 car.), ids 128 bits (22 car.), CSPRNG ; pragmas `WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000`, pool écriture 1, lecture 4 ; `Timestamp` forme unique `YYYY-MM-DDTHH:MM:SSZ` ; en-têtes posés par le binaire (CSP exacte d'AD-9, `Referrer-Policy: no-referrer`, `nosniff`, `Permissions-Policy` minimale, `Cache-Control` différencié, `X-App-Build` sur `/api/*`) ; `cargo check` passe sans build web.

**Never:** aucun écran métier (page vide remplacée en 1-5) ; aucune table métier ; aucun chargeur `.env` ; aucune crate client réseau (le `healthcheck` écrit sa requête à la main sur `TcpStream`) ; aucun journal d'URI, de corps ni d'en-tête de requête (les chemins SPA porteront des secrets) ; aucun `datetime('now')` ni `CURRENT_TIMESTAMP` ; registre d'erreurs problem+json (1-5), référence (1-2), Dockerfile et CI (1-3) hors périmètre.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Santé OK | base ouverte, migrations à jour | `GET /api/health` → 200, `no-store`, `X-App-Build` | N/A |
| Santé KO | pool fermé ou migration embarquée non appliquée | 503 sans détail | cause en `tracing` niveau `warn` |
| Route SPA | `GET /e/abc`, `GET /` | 200 `index.html`, `Cache-Control: no-cache` | N/A |
| Asset | `GET /assets/x-HASH.js` présent / absent | 200 `immutable` / 404 | N/A |
| API inconnue | `GET /api/nope` | 404, jamais `index.html` | N/A |
| Config | `PUBLIC_BASE_URL` absent, `FORM_TOKEN_SECRET` < 32 octets, défaut mal formé | sortie code ≠ 0 au démarrage, nom de la variable (jamais sa valeur) sur stderr | fail-fast |
| Timestamp | `2026-11-06T10:00:00Z` / `+00:00`, fraction, sans `Z` | accepté / rejeté | erreur de parsing |
| healthcheck | serveur 200 / 503 / arrêté | code 0 / ≠ 0 / ≠ 0 | délai borné |

**Décisions (Vincent, 2026-11-06) :**
- Chemins de données fixes `/data/...` partout, sans variable ni repli selon le profil : Vincent ne lance jamais le binaire hors conteneur, il vérifie en ligne après push. Les tests n'écrivent jamais dans `/data` (base mémoire ou fichier temporaire injecté).
- Spec gardée entière malgré ~2 200 tokens.

</frozen-after-approval>

## Code Map

- Dépôt vierge de code : seulement `docs/`, `_bmad*`, `.gitignore`. Rien à réutiliser.
- `.gitignore` -- contient `dist/`, qui empêcherait de versionner `web/dist/.gitkeep`.
- `_bmad-output/specs/spec-epic-1-socle/configuration.md` -- variables et défauts que `Config` doit lire.
- Spine *Structural Seed* -- arborescence cible ; *Stack* -- versions exactes (vérifiées disponibles sur crates.io et npm).
- Toolchain locale : rustc 1.96, Node 24.18 ; la spine fixe Rust 1.99.1.

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.toml`, `rust-toolchain.toml` -- workspace (édition 2024, `resolver = "3"`), dépendances communes en `[workspace.dependencies]`, toolchain 1.99.1 avec `rustfmt`, `clippy` -- versions figées.
- [x] `crates/domain/` -- `Timestamp` (newtype chrono UTC, `FromStr`/`Display`/serde stricts, `sqlx::Type` derrière la feature optionnelle `sqlx`), tests unitaires -- seul type d'instant.
- [x] `crates/server/src/infra/{clock,ids,sqlite}.rs` -- traits `Clock`, `IdGenerator` (impl. système et déterministe de test), `Db { write, read }`, pragmas, `sqlx::migrate!` -- persistance et injection.
- [x] `crates/server/migrations/.gitkeep` -- dossier vide, aucune migration métier.
- [x] `crates/server/src/config.rs` -- `Config::from_env` sur une source injectable, toutes les variables epic 1 de `configuration.md` -- fail-fast testable.
- [x] `crates/server/src/app/health.rs` -- `health(&AppState) -> Health` (base ouverte, migrations à jour) -- source unique de la santé.
- [x] `crates/server/src/http/{mod,headers,assets}.rs` -- routeur `/api/health`, fallback assets/SPA, couche d'en-têtes, `X-App-Build` -- même origine, pas de CORS.
- [x] `crates/server/src/main.rs`, `crates/server/build.rs` -- sous-commandes `serve` (défaut, écoute `0.0.0.0:8080`, arrêt propre sur SIGTERM) et `healthcheck` ; hash de build depuis `APP_BUILD` à la compilation, `dev` sinon -- AD-1, AD-8.
- [x] `web/` -- `package.json` (versions exactes), `package-lock.json`, Vite + React + TS, page vide ; `web/dist/.gitkeep` ; `.gitignore` corrigé -- AD-1.
- [x] `crates/server/tests/` -- tests HTTP de la matrice, test de pragmas et tailles de pools, test grep `datetime('now')`/`CURRENT_TIMESTAMP` sur le dépôt -- CAP-1, CAP-2.

**Acceptance Criteria:**
- Given un dépôt sans `web/dist` construit, when `cargo check` puis `cargo test`, then les deux passent.
- Given `npm run build` dans `web/` puis `cargo build`, when un test HTTP demande `/`, then la page vide embarquée est servie avec la CSP exacte d'AD-9.
- Given le `Cargo.lock`, when `cargo tree -e features`, then ni `reqwest`, `ureq`, `lettre`, `hickory`, ni `hyper-util` feature `client`.
- Given `cargo clippy --all-targets -- -D warnings` et `cargo fmt --check`, then aucun avertissement.

## Design Notes

`Timestamp` doit implémenter `sqlx::Type` (AD-11) mais `domain` ne dépend pas de sqlx (Design Paradigm), et la règle d'orphelin interdit l'impl. dans `server` : feature Cargo optionnelle `sqlx` dans `domain`, activée par `server` seulement ; les tests de `domain` restent sans sqlx.

Tests HTTP sur une base mémoire partagée nommée par test (`file:<nom>?mode=memory&cache=shared`), les deux pools pointant dessus ; le test de pragmas `WAL` utilise un fichier temporaire (WAL n'existe pas en mémoire).

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` -- expected: vert, sans `web/dist` construit.
- `cd web && npm ci && npx tsc --noEmit && npm run build` -- expected: `web/dist/index.html` produit.
- Test d'intégration qui lance le serveur sur un port éphémère et un fichier temporaire, puis appelle la fonction du `healthcheck` (adresse injectable, `127.0.0.1:8080` fixe dans `main`) -- expected: 0 sur 200, ≠ 0 sur 503 et serveur arrêté. Aucun `cargo run` sur le poste (`/data` absent).

## Implementation Notes

- Router générique sur son dossier d'assets (`router_with::<A>`) : les tests HTTP servent une SPA de démonstration (`crates/server/tests/fixtures/dist`), indépendante du build Vite.
- `web/public/.gitkeep` recopié par Vite dans `web/dist/` à chaque build, sinon `emptyOutDir` supprime le `.gitkeep` versionné.
- `rust-embed` avec `debug-embed` : les tests debug exercent l'embarquement réel.
- `RUST_LOG` limité à `off`/`error`/`warn`/`info` : le binaire ne tourne qu'en production.
- `AppState` ne porte pas encore `Config` : à ajouter en 1-6/1-7, premiers consommateurs.
- Ajouts hors table Stack, en dev-dependencies seulement : `serde_json` 1.0.154 (domain), `tower` 0.5.3 (server).

## Spec Change Log

## Review Triage Log

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | blind | `Config` validée puis inutilisée, `PORT` validé alors qu'ignoré par le binaire | low | voulu : fail-fast au démarrage, consommateurs en 1-6/1-7 ; un `PORT` faux dans `.env` casserait aussi le compose | rejeté |
| 2 | blind, edge | HEAD renvoie `content-length: 0` | low | `Body::empty()` sur HEAD, sans en-tête de longueur | patch |
| 3 | blind | pas d'`ETag` sur `index.html` | low | page de 0,3 ko en `no-cache` ; gain négligeable, ajoute une branche | rejeté |
| 4 | blind, edge | sans `debug-embed`, les tests debug lisent le disque au lieu d'embarquer | medium | rust-embed lit le dossier à l'exécution en debug ; seul un test release prouvait l'embarquement | patch |
| 5 | blind | `POST /api/` → 405 au lieu de 404 | low | fallback SPA teste la méthode avant `is_api` | patch |
| 6 | blind, edge | garde AD-11 contournable (`date('now'`, `unixepoch(`, tabulation…) | medium | seuls les espaces sont retirés, trois motifs seulement | patch |
| 7 | blind | `PartialEq` non constant sur `Secret` | false | AD-3 cherche par hash en SQL, aucun secret n'est comparé en Rust | rejeté |
| 8 | blind | santé n'interroge que le pool de lecture | low | « base ouverte » satisfait ; interroger l'unique connexion d'écriture ferait attendre la santé derrière les commandes | rejeté |
| 9 | blind | `deploy/.env.example` absent | low | prévu par `stories.yaml` en 1-3 | defer |
| 10 | blind | `TRUST_CLIENT_IP_HEADER` vrai par défaut | false | défaut fixé par `configuration.md` ; port lié à `127.0.0.1` (AD-8) | rejeté |
| 11 | blind, edge | `FORM_TOKEN_SECRET` : longueur seule, pas d'entropie | low | règle de `configuration.md` respectée ; contrôle d'entropie = complexité sans erreur réaliste | rejeté |
| 12 | blind, edge | `parse_base_url` accepte `https://:443`, un chemin | low | erreur d'exploitant improbable ; aucun consommateur avant l'epic 2 | rejeté |
| 13 | blind, verif | `healthcheck::ADDR` code 8080 en dur, hors `LISTEN_PORT` | low | deux constantes indépendantes | patch |
| 14 | blind | `package-lock.json` absent | false | présent dans l'arbre, exclu du diff de revue seulement | rejeté |
| 15 | blind | `Content-Type` sans `charset` | low | `<meta charset="utf-8">` et modules JS UTF-8 par défaut | rejeté |
| 16 | verif | branche `MigrationLag::Mismatch` non testée | medium | aucun test n'insère de checksum faux ni `success = 0` | patch |
| 17 | verif | dispatch des sous-commandes et codes de sortie non testés au niveau processus | medium | aucun `CARGO_BIN_EXE` ; le consommateur est le Dockerfile | defer |
| 18 | verif | `embedded_web_dist` se replie en silence sans build web | medium | ne peut être rendu strict qu'en CI après `npm run build` | defer |
| 19 | edge | `ctrl_c()` en erreur arrête le serveur aussitôt | low | `let _ =` puis le `select!` se résout | patch |
| 20 | edge | drain sans échéance au SIGTERM | low | Docker tue après son délai, WAL garantit la cohérence | rejeté |
| 21 | edge | délai du healthcheck par lecture, pas global | low | pair local seulement, lecture arrêtée au premier `\n` | rejeté |
| 22 | edge | `EINTR` sur `read` | low | processus court sans gestionnaire de signal | rejeté |
| 23 | edge | santé bloquée 30 s si le pool de lecture est saturé | low | le healthcheck abandonne à 3 s et Docker borne ; 4 connexions pour une dizaine de joueurs | rejeté |
| 24 | edge | pool de lecture non `read_only` | low | aucune écriture via `read` ; AD-16 impose le pool d'écriture aux commandes | rejeté |
| 25 | edge | santé OK sur un schéma plus récent que le binaire | false | `MIGRATOR.run` échoue au démarrage sur une version appliquée inconnue (`VersionMissing`) | rejeté |
| 26 | edge | `MIN_SUBMIT_DELAY_MS` démesuré accepté | low | erreur d'exploitant, consommateur en 1-7 | rejeté |
| 27 | edge | `CLIENT_IP_HEADER=forwarded` accepté | low | valeur choisie par l'exploitant, edge écrase l'en-tête configuré | rejeté |
| 28 | edge | chemins d'assets percent-encodés → 404 | low | Vite ne produit que des noms ASCII sûrs | rejeté |
| 29 | edge | argument non Unicode → panique | low | arguments fixes dans le conteneur | rejeté |
| 30 | edge | `build.rs` ne suit pas `tests/fixtures/migrations` | low | migration de test périmée possible | patch |
| 31 | edge | `dist/` global retiré du `.gitignore` | low | `tests/fixtures/dist` doit être versionné ; aucun autre `dist` prévu | rejeté |
| 32 | edge | `RUST_LOG=debug` accepté alors qu'interdit en production | medium | le binaire ne tourne qu'en production (décision du 2026-11-06) | patch |
