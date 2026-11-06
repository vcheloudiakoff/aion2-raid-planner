---
title: 'Droits par liens secrets'
type: 'feature'
created: '2026-11-06'
status: 'done'
baseline_revision: '03d74569dd059873744d846b43a2512d9d576c08'
review_loop_iteration: 0
followup_review_recommended: false
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** Sans compte, chaque droit (organisateur `/o/`, candidature directe `/j/`, participant `/p/`) n'est porté que par un secret de lien. Les epics 2 à 5 ont besoin d'un mécanisme unique et sûr pour le vérifier. Ce mécanisme ne doit jamais garder le secret en clair, ni le lire ailleurs que dans un en-tête (SPEC CAP-8, AD-3).

**Approach:**
- Côté serveur : une table `links` qui ne stocke que `SHA-256(secret)`, et un extracteur Axum unique `LinkSecret<R>` qui résout le droit déclaré par la route et la portée.
- Le chemin de la SPA reçoit `X-Robots-Tag: noindex`.
- Côté client : un module `links.ts` sur `links.v1` et une option `linkSecret` dans `client.ts`.
- Preuve par des routes compilées sous `cfg(test)` seulement.

## Boundaries & Constraints

**Always:**
- **Format du secret :** 256 bits, base64url sans padding, 43 caractères, produit par l'`IdGenerator` existant. Les ids publics sont de 128 bits, 22 caractères.
- **Stockage :** la base ne contient que `SHA-256(secret)`, sur 32 octets. La recherche se fait par le hash, sur le pool de lecture.
- **Lecture du secret :** uniquement dans l'en-tête `X-Link-Secret`. La query string et les cookies ne sont jamais lus.
- **Ordre des réponses :**
  - en-tête absent : 401 `auth.missing` ;
  - valeur mal formée, en double ou inconnue : 404 `auth.not_found` ;
  - secret d'un autre droit, ou d'une autre portée (`event_id`, ou `candidature_id` pour le participant) : 403 `auth.wrong_right`.
- **Journaux :** l'extracteur ne journalise jamais la valeur ni le hash (AD-15).
- **Codes d'erreur :** les trois codes `auth.*` entrent dans le registre `http::errors::Code`, et dans `fr.json` et `en.json`.
- **Chemins de la SPA :** `/o`, `/j`, `/p` et leurs sous-chemins portent `X-Robots-Tag: noindex`.
- **Client :** `links.ts` passe par `lib/storage.ts`. Un `localStorage` absent ou qui lève une exception donne une valeur vide ou une écriture ignorée, jamais une exception remontée.

**Never:**
- Aucune route métier réelle.
- Aucune route de test compilée dans le binaire de production.
- Pas de droit admin ni de `ADMIN_SECRET` : c'est l'epic 5.
- Pas de `noindex` sur `/e/` non listé : c'est l'epic 2, qui connaît `visibility`.
- Pas de clé étrangère vers des tables inexistantes.
- Aucune récupération de lien perdu.
- Aucun secret dans une URL d'API, un cookie ou un journal.
- Pas de nouvelle dépendance, Rust ou npm.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Bon droit, bonne portée | organizer de E, route organizer `/…/events/E/…` | 200, l'extracteur expose `event_id` | — |
| Participant | secret participant (E, C), route `/…/events/E/candidatures/C` | 200, l'extracteur expose E et C | — |
| Absent | pas d'en-tête, même avec `?secret=` ou cookie `X-Link-Secret` | 401 `auth.missing` | problem+json |
| Mal formé ou doublé | 42 caractères, caractère hors base64url, deux en-têtes | 404 `auth.not_found` | sans requête SQL |
| Inconnu | 43 caractères valides absents de la base | 404 `auth.not_found` | — |
| Autre droit | secret `join` sur route organizer | 403 `auth.wrong_right` | — |
| Autre portée | organizer de E sur `/events/F`, participant C sur `/candidatures/D` | 403 `auth.wrong_right` | — |
| Base en erreur | pool fermé | 500 `server.internal` | erreur journalisée sans la valeur du secret |
| Stockage client cassé | `localStorage` lève une exception | `readLinks()` = `{}`, écriture = `false` | aucune exception |
| Stockage client corrompu | JSON invalide, entrée au mauvais type | entrées invalides ignorées, valides conservées | — |

</intent-contract>

## Code Map

- `crates/server/src/infra/ids.rs` -- `IdGenerator::secret()`, `Secret::expose`, `SECRET_LEN=43` et `ID_LEN=22` existent déjà. À réutiliser, rien à refaire.
- `crates/server/migrations/` -- vide (`.gitkeep`). Première migration du dépôt ; `MIGRATOR` dans `infra/sqlite.rs:20`. Les migrations sont immuables une fois poussées.
- `crates/server/src/infra/sqlite.rs` -- `Db {write, read}`, `shared_memory_options(name)`, `Db::open`, `migrate`.
- `crates/server/src/http/errors.rs` -- macro `codes!` (ajouter les variantes), `ENTITIES` contient déjà `auth`, `impl IntoResponse for Code`.
- `crates/server/src/http/mod.rs` -- `router_with` ; aucune route à ajouter en production.
- `crates/server/src/http/headers.rs` -- middleware `security_headers` et `is_api`. Il ne journalise rien : à garder ainsi.
- `crates/server/src/app/mod.rs` -- `AppState { db, clock, ids, reference, migrator }`.
- `crates/server/tests/common/mod.rs` -- `memory_state`, `fixture_router`, `send_with` pour les tests d'intégration. Les routes de test vivent dans les tests unitaires du crate `server`, car `cfg(test)` ne s'applique pas aux tests de `tests/`.
- `crates/server/Cargo.toml` -- `sha2`, `data-encoding` et `tower` (dev) sont déjà présents.
- `web/src/lib/storage.ts` -- `readJson` et `writeJson` enveloppés. Mettre à jour son commentaire d'en-tête : `links.ts` passe par lui.
- `web/src/api/client.ts` -- `getConditional(url)` et `getJson(url)` construisent `headers` ligne 89.
- `web/src/locales/fr.json`, `en.json` -- clés `errors.*` ; `web/scripts/check-error-codes.mjs` vérifie les deux sens.

## Tasks & Acceptance

**Execution:**
- `crates/server/migrations/0001_links.sql` -- créer la table `links`, `STRICT, WITHOUT ROWID`, avec :
  - `secret_hash BLOB PRIMARY KEY CHECK(length = 32)` ;
  - `link_right TEXT CHECK IN ('organizer','join','participant')` ;
  - `event_id TEXT NOT NULL` et `candidature_id TEXT`, chacun avec `CHECK(length = 22)` ;
  - `CHECK((link_right = 'participant') = (candidature_id IS NOT NULL))` ;
  - un index sur `event_id`, pour la purge future.
  -- Rationale : une table unique permet de distinguer 403 de 404 en une seule recherche.
- `crates/server/src/infra/links.rs` (+ `infra/mod.rs`) -- `Right` (`as_str`, conversion depuis le SQL), `hash(&str) -> [u8; 32]`, `LinkRow { right, event_id, candidature_id }`, `insert(&mut SqliteConnection, &Secret, &LinkRow)` et `find(&SqlitePool, &[u8; 32]) -> Result<Option<LinkRow>>`. Tests unitaires : le hash est stocké, jamais le clair. -- Seul SQL des liens.
- `crates/server/src/http/errors.rs` -- `AuthMissing => "auth.missing", UNAUTHORIZED`, `AuthNotFound => "auth.not_found", NOT_FOUND`, `AuthWrongRight => "auth.wrong_right", FORBIDDEN`. -- Registre unique.
- `crates/server/src/http/links.rs` (+ `http/mod.rs`) -- trait `RequiredRight { const RIGHT: Right }`, marqueurs `Organizer`, `Join`, `Participant`, et `LinkSecret<R>: FromRequestParts<AppState>`, selon l'ordre de la matrice.
  - La portée se lit dans `RawPathParams` : `event_id` est obligatoire ; `candidature_id` est comparé pour le participant quand la route le déclare.
  - Une route sans `event_id` est une erreur de programmation : 500 `server.internal` et `debug_assert`.
  - Accesseurs `event_id()` et `candidature_id()`.
  - Le module `#[cfg(test)]` monte des routes de test sur une base mémoire et couvre toute la matrice serveur, avec `tower::ServiceExt::oneshot`. -- Extracteur unique d'AD-3.
- `crates/server/src/http/headers.rs` -- ajouter `X-Robots-Tag: noindex` hors `/api` quand le chemin vaut `/o`, `/j`, `/p` ou commence par `/o/`, `/j/`, `/p/` (pas `/old`). -- Chemins secrets non indexés.
- `crates/server/tests/http.rs` -- tester `X-Robots-Tag` sur `/o/<secret>`, `/j/x`, `/p/x`, et son absence sur `/`, `/old` et `/api/health`. -- Surface HTTP réelle.
- `web/src/api/links.ts` -- clé `links.v1`. Types `EventLinks { organizer?, join?, participants: Record<candidatureId, secret> }` et `Links`. Fonctions :
  - `readLinks()`, qui assainit : clés d'événement de 22 caractères, secrets de 43 caractères base64url ; entrée invalide ignorée ;
  - `eventLinks(id)` ;
  - `saveOrganizerLinks(id, { organizer, join })`, `saveParticipantLink(id, candidatureId, secret)`, qui renvoient `boolean` ;
  - `forgetParticipant`, `forgetEvent`.
- `web/src/api/links.test.ts` -- tests vitest :
  - aller-retour ;
  - fusion sans écraser les autres entrées ;
  - données corrompues ;
  - `localStorage` qui lève une exception, avec la page toujours fonctionnelle (`readLinks()` vide, écriture `false`).
- `web/src/api/client.ts` (+ test) -- option `{ linkSecret?: string }` sur `getConditional` et `getJson`, qui pose `X-Link-Secret` seulement si elle est fournie. Le test vérifie l'en-tête et l'absence du secret dans l'URL. -- Le client n'envoie que le secret qu'on lui passe.
- `web/src/locales/fr.json`, `en.json` -- `errors.auth.missing`, `errors.auth.not_found`, `errors.auth.wrong_right`, sur le ton d'EXPERIENCE.md :
  - absent : « Il faut le lien complet pour ouvrir cette page. » ;
  - inconnu : « Ce lien ne mène à rien : il a peut-être expiré. » ;
  - autre droit : « Ce lien ne donne pas accès à cette page. »

**Acceptance Criteria:**
- Given la base après migrations, when un lien est inséré par `infra::links::insert`, then `links` contient 32 octets égaux à `SHA-256(secret)`, et aucune colonne ne contient la valeur en clair.
- Given le binaire de production, when on liste ses routes, then aucune route de test n'existe : elles sont sous `#[cfg(test)]`.
- Given `cargo run -q -p server -- list-error-codes`, when il passe dans `check-error-codes.mjs`, then les trois codes `auth.*` sont présents dans les deux locales.

## Spec Change Log

## Review Triage Log

Passe 1 : 2 medium, 10 low, 1 false, 0 maybe-false (16 constats, regroupés).

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | edge, blind, verif | mémoire ETag de `client.ts` par URL seule : deux secrets sur une URL partagent `If-None-Match` et corps | medium | `memo.get(url)` sans secret ; clé URL + secret, test à deux secrets | patch |
| 2 | verif | retour 500 de `undeclared_scope` jamais vérifié en CI (le `debug_assert` panique d'abord) | medium | CI sans `--release` ; helper `undeclared_scope_code()` testé | patch |
| 3 | edge, blind | `linkSecret: ""` envoie un en-tête vide → 404 au lieu de 401 | low | test `!== undefined` ; chaîne vide = absente | patch |
| 4 | edge, blind | portée non vérifiée pour `candidature_id` en organizer/join, et route participant sans `candidature_id` | low | contrat implicite ; règles écrites dans la doc du module | patch |
| 5 | blind | test « sans requête » limité à une valeur | low | doublon et caractère hors base64url ajoutés sur pool fermé | patch |
| 6 | edge | `getItem` illisible mais `setItem` OK → écrasement | low | données déjà illisibles, rien de lisible perdu ; cas improbable, garde = complexité | rejeté |
| 7 | edge | entrées assainies perdues à l'écriture | low | schéma versionné (`links.v2` = nouvelle clé) ; entrée invalide = déchet | rejeté |
| 8 | edge, blind | course entre onglets, pas d'abonnement aux changements | low | lecture-écriture synchrone dans un même tick ; aucun écran consommateur avant l'epic 2 | rejeté |
| 9 | edge | `//o/x` sans `X-Robots-Tag` | low | les liens émis sont `/o/…` ; aucun robot ne devine un secret | rejeté |
| 10 | blind | 401 sans `WWW-Authenticate` | low | aucun client affecté ; schéma maison = surface nouvelle | rejeté |
| 11 | blind | test d'erreur base ne couvre pas le décodage | low | `CHECK` + `STRICT` rendent un droit inconnu inatteignable | rejeté |
| 12 | blind | « peut-être expiré » alors que les liens n'expirent pas | false | les liens disparaissent avec la purge de l'événement (AD-14) | rejeté |
| 13 | blind | `noindex` sans `nofollow` | low | valeur fixée par AD-3 ; changement = écart à la spine | rejeté |
| 14 | blind | `forget*` moins testés | low | suppression d'une clé absente sans effet | rejeté |
| 15 | blind | collision de version des migrations de fixture non gardée | low | `9999` loin de toute migration réelle | rejeté |

## Design Notes

**Une table `links`, plutôt que des colonnes de hash sur `events` et `candidatures` :**
- Pour distinguer 403 (« ce secret existe mais pour un autre droit ») de 404, il faut retrouver le hash quel que soit le droit. Une table donne cette réponse en une seule recherche par clé primaire.
- Les tables métier n'existent pas encore (epic 2). La table n'a donc pas de clé étrangère : la suppression d'un événement (annulation, purge en epic 5) devra supprimer ses liens dans la même transaction, d'où l'index sur `event_id`.

**Une valeur mal formée donne 404 sans requête SQL :** elle est indiscernable d'un secret inconnu pour le porteur, et ça évite une recherche inutile.

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` -- expected: vert.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: code 0.
- `cd web && npx tsc --noEmit && npx vitest run && npm run build && node scripts/check-budget.mjs` -- expected: vert, budget respecté.
- `bash deploy/tests/plan-deploy.test.sh` -- expected: vert, la première migration ajoutée ne casse pas le plan de déploiement.

## Auto Run Result

Status: done

**Résumé :** droits par liens secrets. Table `links` (seul `SHA-256(secret)`), extracteur `LinkSecret<Organizer|Join|Participant>` qui renvoie 401/404/403 selon AD-3, `X-Robots-Tag: noindex` sur `/o`, `/j`, `/p`, module `links.ts` sur `links.v1`, option `linkSecret` de `client.ts`, trois codes `auth.*` traduits.

**Fichiers :**
- `crates/server/migrations/0001_links.sql` -- première migration : table `links`.
- `crates/server/src/infra/links.rs` -- `Right`, `hash`, `insert`, `find`.
- `crates/server/src/http/links.rs` -- extracteur et routes de test `cfg(test)`.
- `crates/server/src/http/errors.rs` -- `auth.missing`, `auth.not_found`, `auth.wrong_right`.
- `crates/server/src/http/headers.rs` -- `X-Robots-Tag` sur les chemins secrets.
- `crates/server/tests/{http,sqlite}.rs`, fixture `9999_fixture_probe.sql` -- tests adaptés à la première migration.
- `web/src/api/links.ts` (+ test), `web/src/api/client.ts` (+ test), `web/src/lib/storage.ts`, locales fr/en.

**Revue :** 5 patchs (2 medium, 3 low), 0 différé, 10 rejetés (raisons dans le triage).

**Relecture complémentaire recommandée :** non (un seul `high` aurait suffi, deux `medium` patchés ne sont pas un risque non vérifié nommable : chacun a son test).

**Vérification :** `cargo fmt --check`, `clippy -D warnings`, `cargo test` verts ; `list-error-codes` → 8 codes dans 2 locales ; `tsc`, vitest 80/80, build, budget JS 82,8/150 ko et CSS 3,4/20 ko ; `plan-deploy.test.sh` 20/20.

**Risques résiduels :** la migration `0001_links.sql` devient immuable dès le push ; la suppression d'un événement devra supprimer ses liens dans la même transaction (aucune clé étrangère).
