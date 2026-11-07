---
title: 'Créer un événement et recevoir ses liens'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '024b60fa7ae5b8bf2ecf57fb761a8e2d49586edb'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/commandes-et-droits.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** l'outil n'a encore aucun modèle métier. Un organisateur ne peut pas publier de raid ni obtenir les liens qui lui donnent ses droits, sans compte (SPEC CAP-1, CAP-7 pour ces écrans).

**Approach:**
- **Migration `0002` :** elle pose tout le modèle de l'epic 2 en une fois, parce qu'une migration est immuable une fois poussée (AD-2).
- **Commande `create_event` :** une transaction derrière `POST /api/events`, sous jeton de formulaire et honeypot, classe de limiteur `create`, disjoncteur `Trip::Events`.
- **Écran `/new` :** un seul écran à 360 px, suivi de l'étape « Liens créés » et de ses trois puces.
- **Contrat d'erreur :** les problem+json gagnent `fields`, et un corps JSON mal formé devient un problem+json (report de 1-7).

## Boundaries & Constraints

**Always:**

- **Migration `crates/server/migrations/0002_events.sql` :** `STRICT`, `foreign_keys` actifs, instants en `TEXT` au format `domain::Timestamp` (`CHECK (length(x) = 20)`), ids sur 22 caractères.
  - `events` :
    - colonnes `id` PK, `instance`, `region` ;
    - `faction_scope` ∈ `all`/`elyos`/`asmodian`, `visibility` ∈ `listed`/`unlisted`, `status` ∈ `open`/`locked`/`cancelled` ;
    - `min_cp` INTEGER NULL, `discord_url` NULL, `note` NOT NULL DEFAULT `''` ;
    - `created_at`, `cancelled_at` NULL, `admin_unlisted_at` NULL.
  - `slots` :
    - colonnes `id` PK sur 22 caractères, `event_id`, `starts_at`, `duration_min` INTEGER > 0 ;
    - `event_id` en clé étrangère vers `events`, `ON DELETE CASCADE`, avec un index sur `event_id`.
  - `class_bounds` :
    - colonnes `event_id`, `class`, `min_count`, `max_count`, avec `CHECK (0 <= min_count AND min_count <= max_count)` ;
    - PK `(event_id, class)`, clé étrangère `ON DELETE CASCADE` ;
    - une ligne par classe `enabled` au moment de la création.
  - `candidatures` :
    - colonnes `id` PK sur 22 caractères, `event_id` (clé étrangère, cascade), `pseudo`, `server`, `class`, `cp` INTEGER ;
    - `status` ∈ `pending`/`accepted`/`rejected`/`withdrawn`, `origin` ∈ `direct`/`public` ;
    - `created_at`, `updated_at`, index sur `event_id`.
  - `availabilities` :
    - colonnes `candidature_id`, `slot_id`, `preferred` INTEGER `CHECK (preferred IN (0,1))` ;
    - PK `(candidature_id, slot_id)`, les deux clés étrangères en `ON DELETE CASCADE`, index sur `slot_id`.
  - Jamais de `CHECK` sur la longueur ou le charset du pseudo, ni sur les bornes du CP : ces valeurs ne sont pas encore fixées.
- **Entrée de `create_event`** (corps `FormEnvelope<CreateEvent>`, camelCase) :
  - `instance`, `region`, `factionScope` ;
  - `slots[{startsAt, durationMin?}]`, où `durationMin` vaut par défaut le `duration_min` de l'instance ;
  - `note?`, `discordUrl?`, `visibility` (défaut `listed`), `minCp?` ;
  - `classBounds?` : une map `classId → {min, max}`. Une classe omise prend `default_min`/`default_max`.
- **Validation** pure dans `domain` (les fonctions de `domain::validate` sont réutilisées) :
  - `instance` et `region` existent et sont `enabled` ;
  - si l'instance a `cross_faction = false`, `factionScope = all` est refusé `[ASSUMPTION]` ;
  - de 1 à 14 créneaux, pas deux fois le même `startsAt`, `startsAt` strictement après `clock.now()` ;
  - `durationMin` de 15 à 720 `[ASSUMPTION]` ;
  - `note` et `discordUrl` passent par les fonctions existantes ; `minCp` passe par `cp` ;
  - pour chaque borne : la classe existe et est `enabled`, et `0 ≤ min ≤ max ≤ parties × party_size` de l'instance.
- **Erreurs** : toutes les erreurs de champ sont collectées.
  - La réponse est 400. `code` est celui de la première erreur, dans l'ordre des champs ci-dessus. `fields` associe chaque chemin JSON camelCase (`slots.2.startsAt`, `classBounds.cleric.max`) à son code complet.
  - Nouveaux codes, tous en 400 :
    - `form.malformed` ;
    - `reference.unknown` ;
    - `event.faction_scope_not_allowed` ;
    - `slot.in_past`, `slot.count_out_of_range`, `slot.duplicate`, `slot.duration_out_of_range` ;
    - `event.note_invalid`, `event.discord_url_invalid`, `event.min_cp_out_of_range`, `event.class_bounds_invalid`.
  - Chaque code a son `errors.<code>` dans `fr.json` et `en.json`.
- **Ordre dans la commande :**
  1. `FormEnvelope::check` (honeypot puis jeton, classe `Create`) ;
  2. validation ;
  3. ouverture de la transaction sur `db.write` ;
  4. `breaker.check(Trip::Events)` ;
  5. insertions `events`, `slots`, `class_bounds`, puis deux `links` (`organizer`, `join`) par `infra::links::insert` ;
  6. commit ;
  7. `breaker.record(Trip::Events)`.
- **`CreatedEvent`** (`http::dto`, miroir à la main dans `web/src/api/types.ts`) :
  - forme : `{eventId, organizerSecret, joinSecret, links: {organizer: "/o/<s>", join: "/j/<s>", public: "/e/<id>"}}` ;
  - chemins relatifs : la SPA les préfixe avec `location.origin` ;
  - réponse 201 avec `Cache-Control: no-store`.
- **Rejet JSON :** tout rejet `Json` d'axum sur une route de commande renvoie problem+json 400 `form.malformed`. Le secret n'apparaît jamais dans les journaux.
- **Écran `/new`** : un seul écran à 360 px, sans défilement horizontal.
  - Champs : instance, région, portée (« Toutes » ou une faction), créneaux en date et heure natives avec durée, « + Ajouter un créneau ».
  - Ensuite : note (compteur sur 280), lien Discord, « Lister publiquement » coché par défaut.
  - « Options » est replié et contient le CP minimum et les bornes par classe, en Stepper −/+ de 44 px, préremplies depuis la référence.
  - Bouton unique « Publier », libellé « Envoi… » pendant l'envoi, raison de désactivation affichée sous le bouton.
  - Le fuseau du navigateur est affiché une fois. Les heures locales sont converties en UTC avant l'envoi.
  - Les libellés de jeu viennent de la référence.
- **Jeton côté web :**
  - il est demandé en parallèle au premier rendu ;
  - sur `form.token_invalid`, la SPA redemande un jeton et réessaie une fois, sans rien dire ;
  - sur `form.too_fast`, elle attend `MIN_SUBMIT_DELAY` (3 s), puis réessaie une fois ;
  - toute autre erreur affiche `errors.<code>`, avec la saisie conservée et les erreurs de `fields` sous leurs champs.
- **Étape « Liens créés »** (sur `/new`, après le succès) :
  - `saveOrganizerLinks` est appelé dès le succès, avant l'affichage ;
  - trois `copy-chip`, dans l'ordre : « Ton lien d'organisateur » avec l'avertissement « Garde ce lien : c'est le seul moyen de gérer ton raid. » et un bouton « Partager » quand Web Share existe ; « Lien direct » ; « Lien public » ;
  - un toucher copie et affiche le toast « Copié » 1,5 s ; si le presse-papiers échoue, le texte est sélectionné ;
  - « Ouvrir mon raid » (vers `/o/<secret>`) reste désactivé tant que le lien d'organisateur n'est ni copié, ni partagé, ni déclaré gardé par la case « J'ai gardé mon lien » ;
  - si `saveOrganizerLinks` renvoie false : « Ce navigateur ne gardera pas le lien : copie-le maintenant. » ;
  - le secret n'apparaît jamais dans `document.title`.
- **Accueil :** il gagne un bouton « Créer un raid » vers `/new`.
- **Route `/o/:secret` :** elle affiche une page d'attente à texte localisé, remplacée en 2-4.
- **Locales :** tous les textes sont en fr et en en. Ton *Voice and Tone* : tutoiement, pas de point d'exclamation, pas d'excuses.

**Never:**

- Créer les tables `groups`, `assignments` ou `confirmations`.
- Modifier `0001_links.sql` ou la fixture `9999`.
- Stocker la faction ou le rôle.
- Ajouter une route de lecture (`public_event`, `organizer_view`), une candidature ou l'interrupteur admin `creation_enabled` : ce sont les stories 2-2 et 2-4, et l'epic 5.
- Mettre un secret dans une URL d'API.
- Ajouter une dépendance réseau.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Création nominale | jeton de plus de 3 s, Ludra, 3 créneaux futurs, sans bornes | 201 `CreatedEvent` ; 1 `events` en `open`, 3 `slots` à la durée de l'instance, une `class_bounds` par classe active, 2 `links` | — |
| Créneau passé | `slots.1.startsAt` ≤ now | rien n'est écrit | 400 `slot.in_past`, `fields["slots.1.startsAt"]` |
| Plusieurs erreurs | note avec URL et région inconnue | rien n'est écrit | 400 `code=reference.unknown`, `fields` a 2 clés |
| Créneaux hors bornes | 0 ou 15 créneaux, ou deux fois le même `startsAt` | rien n'est écrit | 400 `slot.count_out_of_range` ou `slot.duplicate` |
| Bornes | `min > max` ou `max > parties×party_size` | rien n'est écrit | 400 `event.class_bounds_invalid` |
| Portée interdite | `all` sur une instance `cross_faction=false` | rien n'est écrit | 400 `event.faction_scope_not_allowed` |
| Honeypot ou jeton | `website` rempli, jeton absent ou trop jeune | rien n'est écrit | 400 `form.rejected`, `form.token_invalid` ou `form.too_fast` |
| JSON mal formé | corps `{` ou champ de mauvais type | problem+json | 400 `form.malformed` |
| Disjoncteur | `MAX_EVENTS_PER_HOUR` atteint | rien n'est écrit | 503 `limit.paused` + `Retry-After` |
| Stockage indisponible | `localStorage` qui lève | Liens créés s'affiche quand même avec l'avertissement « copie-le maintenant » | — |

</intent-contract>

## Code Map

- `crates/server/migrations/0001_links.sql` -- modèle de style (STRICT, `CHECK length=22`). Lecture seule. Les migrations sont embarquées par `infra/sqlite.rs:20` (`MIGRATOR`) et `deploy/plan-deploy.sh` impose qu'elles soient immuables.
- `crates/server/src/infra/sqlite.rs:24` -- `Db {write (1 conn), read (4)}`, `foreign_keys(true)` à L30.
- `crates/server/src/infra/ids.rs:52` -- `IdGenerator::public_id()` (22 caractères) et `secret()` (43). `Secret::expose()` à L41. `DeterministicIdGenerator` à L86 pour les tests.
- `crates/server/src/infra/clock.rs:7` -- `Clock::now() -> Timestamp`. `FixedClock` sert aux tests ; `tests/no_sql_now.rs` interdit `now()` en SQL.
- `crates/server/src/infra/links.rs:72` -- `insert(&mut SqliteConnection, &Secret, &LinkRow)`, à appeler avec `&mut *tx`. `LinkRow` est à L65.
- `crates/server/src/infra/reference.rs:236` -- `ReferenceStore::get() -> Option<Arc<ReferenceSet>>` ; `None` donne `Code::ReferenceUnavailable`. Aucune aide de recherche : `set.instances`, `classes` et `regions` se parcourent par `id`. Le snapshot est pris une seule fois par commande.
- `crates/domain/src/reference.rs:82` -- `Instance {parties, party_size, duration_min, cross_faction, enabled}` et `Class {role, default_min, default_max, enabled}`.
- `crates/domain/src/validate.rs:5-84` -- `note`, `cp` et `discord_url` sont à réutiliser ; `FieldError` est à L16. Le nouveau module de validation de l'événement va dans `domain`, en code pur.
- `crates/domain/src/timestamp.rs:16` -- `Timestamp` (format unique, `FromStr`, serde).
- `crates/server/src/app/mod.rs:18` -- `AppState` ; la nouvelle commande va dans `app/commands/create_event.rs` (AD-16).
- `crates/server/src/http/mod.rs:37-58` -- `router_with` : ajouter un groupe `/events` avec `route_layer(Guard::new(&state.limits, RouteClass::Create))`, sur le modèle de `abuse_tests.rs:69-91`.
- `crates/server/src/http/form_token.rs:133-146` -- `FormEnvelope<T>` (`flatten`) et `check(&tokens, FormClass::Create, now)`. Le rejet JSON est à mapper vers `form.malformed` (report de 1-7).
- `crates/server/src/http/limits.rs:188-249` -- `breaker.check(Trip::Events)` et `record`, à appeler dans la transaction d'écriture, comme dans `abuse_tests.rs:34-52`.
- `crates/server/src/http/errors.rs:49-99,226` -- `codes!{}`, `ENTITIES` et `Problem`, qui n'a pas encore `fields`. Le test `problem_shape` est à mettre à jour.
- `crates/server/src/http/dto.rs` -- DTO en camelCase.
- `crates/server/src/main.rs:28` -- `list-error-codes`, vérifié par `web/scripts/check-error-codes.mjs`.
- `crates/server/tests/common/mod.rs:85-126` -- `send*` envoie un corps vide. Il faut une aide POST JSON ; `abuse_tests.rs:121-193` (`advance`, `token`, `post`) sert de modèle.
- `web/src/App.tsx:22-26` -- routes de react-router 8 : ajouter `/new` et `/o/:secret`.
- `web/src/api/client.ts:77-119` -- `ApiError`, `problemCode`, en-têtes et hors ligne. Ajouter `postJson` et la lecture de `fields`.
- `web/src/api/links.ts:85` -- `saveOrganizerLinks(eventId, {organizer, join}): boolean`.
- `web/src/api/reference.ts` -- `useReference()`.
- `web/src/api/types.ts:183-196` -- `FormToken` et `Problem`.
- `web/src/i18n/index.ts:74` -- `t(key, vars)`, avec des clés typées par `fr.json`.
- `web/src/locales/locales.test.ts` -- mêmes clés, ni `!` ni excuses.
- `web/src/pages/Home.tsx` et `web/src/lib/storage.ts` -- accueil et accès protégés au stockage.
- `web/scripts/check-budget.mjs:11` -- 150 KB de JS gzip.

## Tasks & Acceptance

**Execution:**
- `crates/server/migrations/0002_events.sql` -- créer les 5 tables d'*Always* -- modèle complet de l'epic 2 en une seule migration immuable.
- `crates/domain/src/validate.rs` -- aligner sur le SPEC de l'epic 2 : `PSEUDO_MAX_CHARS = 16` et `CP_MAX = 999_999`, tests compris -- les bornes du SPEC remplacent les `[ASSUMPTION]` de 1-7.
- `crates/domain/src/event.rs` (nouveau, exporté par `lib.rs`) -- `NewEvent` brut, puis `validate_new_event(&ReferenceSet, now, input) -> Result<ValidEvent, Vec<(path, FieldCode)>>`, avec les tests unitaires de chaque ligne d'erreur de la matrice -- règles pures et testables.
- `crates/server/src/http/errors.rs` -- les 11 nouveaux codes, et `Problem.fields: Option<BTreeMap<String,String>>` sérialisé seulement s'il est présent ; adapter `problem_shape` -- contrat AD-5.
- `crates/server/src/app/commands/{mod.rs,create_event.rs}` -- la transaction dans l'ordre d'*Always* -- AD-16.
- `crates/server/src/http/events.rs` (nouveau) et `http/mod.rs` -- handler `POST /api/events` en classe `Create`, `CreatedEvent` dans `dto.rs`, rejet JSON vers `form.malformed`.
- `crates/server/tests/events_create.rs` et `tests/common/mod.rs` -- aide POST JSON, et tests HTTP de chaque ligne de la matrice côté serveur. Vérifier en base les lignes écrites et le hash des liens (`find` renvoie `organizer` et `join` pour l'event).
- `web/src/api/client.ts`, `types.ts` et `events.ts` (nouveau) -- `postJson`, `fields` dans `ApiError`, `getFormToken`, `createEvent` avec la politique de nouvel essai.
- `web/src/pages/NewEvent.tsx`, `LinksCreated.tsx`, `OrganizerPending.tsx`, `components/CopyChip.tsx` et `Stepper.tsx`, `App.tsx`, `Home.tsx`, `index.css` -- écrans d'*Always*.
- `web/src/locales/fr.json` et `en.json` -- textes des écrans et `errors.<code>` des 11 codes.
- `web/src/**/*.test.tsx` -- tests vitest : le formulaire envoie de l'UTC, nouvel essai une fois sur `form.token_invalid`, `fields` affichés sous les champs, ordre des puces, gating de « Ouvrir mon raid » (copie, partage, case), avertissement quand le stockage est absent, secret absent de `document.title`.

**Acceptance Criteria:**
- Étant donné une base migrée, quand `0002` s'applique sur la base de 1-7, alors les tables `groups`, `assignments` et `confirmations` n'existent pas, et supprimer un `events` supprime en cascade ses `slots`, `class_bounds`, `candidatures` et `availabilities`.
- Étant donné une création réussie, quand on rouvre `/o/<organizerSecret>` et `/j/<joinSecret>`, alors `infra::links::find` résout les deux vers l'`eventId` avec le bon droit, et aucun secret brut n'est stocké.
- Étant donné l'écran Liens créés, quand rien n'est copié, partagé ni coché, alors « Ouvrir mon raid » reste désactivé. Il s'active après l'une des trois actions.
- Étant donné les locales, quand `list-error-codes` est vérifié, alors les 11 codes ont leur traduction fr et en, et `locales.test.ts` passe.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 32 findings — high 0, medium 12, low 19, false 1, maybe-false 0
- findings:
  - `[medium]` `[patch]` Décimales et exposants (CP, durée) envoyés tels quels, refusés en `form.malformed` sans champ marqué — `NewEvent.tsx` n'accepte que des entiers en chiffres et donne la raison `new.reason.integer`.
  - `[medium]` `[patch]` Les bornes réglées survivent au changement d'instance et dépassent la nouvelle taille — elles sont ramenées à la nouvelle taille au changement d'instance, et un test le vérifie.
  - `[low]` `[patch]` Les erreurs de créneaux, indexées par position, restent après une suppression ou une modification — l'erreur d'un champ s'efface quand on le modifie, et `slots.*` quand on retire un créneau.
  - `[low]` `[reject]` Pas de règle sur la somme des minimums par classe — les minimums par défaut sont à 0 et il faut un réglage délibéré ; la règle ajouterait une garde, et la composition revient à l'epic 4.
  - `[low]` `[reject]` Les clés de `classBounds` sont renvoyées sans borne dans `fields` — la réponse n'est pas plus grosse que la requête, qui est limitée par le quota `create` ; aucun dommage pour un tiers.
  - `[low]` `[reject]` Pas d'horizon maximal ni de contrôle de chevauchement des créneaux — un chevauchement est légitime (créneaux alternatifs) ; l'horizon relève du confort et ajouterait une règle non spécifiée.
  - `[low]` `[reject]` Pas de `UNIQUE (event_id, starts_at)` dans la migration — le domaine refuse déjà les doublons ; un index unique peut s'ajouter plus tard par une migration additive.
  - `[medium]` `[reject]` Les liens `/e` et `/j` mènent à l'accueil — hors périmètre : ces pages sont la story 2-2.
  - `[low]` `[reject]` Les liens créés ne s'affichent plus après un retour arrière ou un rechargement — ils sont dans `links.v1` dès le succès ; « Sur cet appareil » est la story 2-6.
  - `[low]` `[reject]` Le client code en dur le délai de 3 s — c'est le défaut de production ; l'exposer créerait une nouvelle surface d'API pour un cas de configuration rare.
  - `[low]` `[patch]` 413, 415 et erreur de syntaxe journalisés pareil — le journal de `FormJson` donne `rejection_status`, jamais le corps.
  - `[medium]` `[patch]` Tests HTTP manquants, dont le quota réel de `POST /api/events` — test `create_route_uses_the_create_quota` ajouté. minCp, Discord et instance inconnue sont déjà couverts par les tests de `domain::event`.
  - `[low]` `[patch]` Ordre de `codes!{}` rompu par les `Event*` — entrées déplacées après `AuthWrongRight`.
  - `[low]` `[reject]` Somme des minimums supérieure à la taille (Edge Case Hunter) — même verdict que la ligne de Blind Hunter ci-dessus.
  - `[low]` `[reject]` `startsAt` très lointain (an 9999) — `Timestamp` s'arrête à l'an 9999 et aucun calcul de date n'existe encore ; à revoir si l'epic 3 en ajoute.
  - `[medium]` `[patch]` Durée décimale vers `form.malformed` — même correctif que les décimales.
  - `[medium]` `[patch]` minCp décimal ou `1e400` — même correctif que les décimales.
  - `[low]` `[patch]` Erreurs de créneau périmées après suppression — même correctif que l'effacement des erreurs.
  - `[medium]` `[patch]` Bornes périmées au changement d'instance — même correctif que les bornes.
  - `[low]` `[reject]` Expiration côté client après un commit, puis double création — c'est rare, et un correctif demanderait une clé d'idempotence, donc une nouvelle surface.
  - `[low]` `[reject]` Heure locale dans le trou du changement d'heure — `datetime-local` ne donne jamais de minutes au-delà de 59 ; le trou n'existe qu'une heure par an, la nuit.
  - `[medium]` `[patch]` Route réelle non vérifiée sous la classe `create` — même correctif que le test de quota.
  - `[medium]` `[patch]` Bornes par défaut plafonnées non testées sur une petite instance — test sur `mono` qui envoie `{min:1,max:4}`.
  - `[low]` `[patch]` Routes `/new` et `/o/:secret` non testées au niveau de `App` — deux tests de routage dans `App.test.tsx`.
  - `[medium]` `[patch]` Bornes périmées après changement d'instance (vérification) — même correctif que les bornes.
  - `[medium]` `[patch]` Décimales et message trompeur (vérification) — même correctif que les décimales.
  - `[low]` `[patch]` Portée « Toutes » présélectionnée sur une première instance mono-faction — la portée est dérivée de l'instance courante dès le premier rendu, et c'est testé.
  - `[low]` `[reject]` Un seul écran à 360 px et Stepper de 44 px non testables en happy-dom — vérification manuelle sur téléphone, notée en risque résiduel.
  - `[low]` `[reject]` Création en moins de 60 s sur téléphone — signal manuel de fin d'epic, risque résiduel.
  - `[medium]` `[patch]` `/new` affiche le texte générique `limit.paused` au lieu de celui d'EXPERIENCE — clé `new.paused` (« La création de raids est en pause. Les raids existants marchent toujours. »), la saisie est conservée.
  - `[low]` `[reject]` `CreatedEvent` recopié à la main entre Rust et TS, sans test de bout en bout — c'est la règle d'AD-5 (miroir à la main dans le même commit).
  - `[false]` `[reject]` Bornes du pseudo et du CP changées hors intention — c'est l'alignement délibéré sur les Assumptions du SPEC de l'epic 2 (16 caractères, 999 999), demandé par la tâche de la spec.

## Design Notes

**Liens relatifs plutôt qu'absolus :** `PUBLIC_BASE_URL` n'est pas dans `AppState`. La SPA connaît déjà son origine, donc des chemins relatifs suffisent et évitent d'étendre l'état. Les secrets bruts sont renvoyés en plus, parce que `links.v1` les stocke.

**`fields` porte des codes complets :** la SPA affiche `t('errors.' + fields[path])` sous le champ. Il n'y a pas de deuxième dictionnaire.

**Une ligne `class_bounds` par classe active :** une modification ultérieure de la référence ne change pas les bornes d'un raid existant. La story 2-5 met ces lignes à jour.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe, `vite build` fait d'abord.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.
- `cargo run -q -p server -- validate-reference reference` -- expected: OK.
- `cargo tree -e features --workspace --target all --locked | sh deploy/check-network-crates.sh` -- expected: OK.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh && sh deploy/tests/plan-deploy.test.sh` -- expected: OK.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert, JS sous 150 KB.

## Auto Run Result

Status: done

**Résumé :** la migration `0002_events.sql` pose tout le modèle de l'epic 2 :
- `events`, `slots`, `class_bounds`, `candidatures` et `availabilities` ;
- suppressions en cascade ;
- pas de `groups`, `assignments` ni `confirmations`.

La commande `create_event`, derrière `POST /api/events`, fait tout en une transaction. Elle vérifie le jeton de formulaire, le honeypot et la classe de limiteur `create`, puis le disjoncteur. Elle crée l'événement, ses créneaux, ses bornes et ses deux liens hachés, et renvoie `CreatedEvent` avec des chemins relatifs.

Les problem+json gagnent `fields`. Un corps JSON mal formé donne `form.malformed`, ce qui solde le report de 1-7. Pseudo et CP sont alignés sur le SPEC de l'epic 2 : 16 caractères, 999 999.

Côté web, cette story livre :
- l'écran `/new` sur un seul écran ;
- l'étape « Liens créés », avec ses trois puces et le blocage de « Ouvrir mon raid » ;
- la page d'attente `/o/:secret` ;
- le bouton « Créer un raid » sur l'accueil ;
- tous les textes en fr et en.

**Fichiers :**
- `crates/server/migrations/0002_events.sql` : le modèle complet de l'epic 2.
- `crates/domain/src/event.rs` : la validation pure de l'événement, avec ses 15 tests.
- `crates/domain/src/validate.rs` : les nouvelles bornes du pseudo (16) et du CP (999 999).
- `crates/server/src/app/commands/create_event.rs` : la transaction de création.
- `crates/server/src/infra/events.rs` : les insertions SQL.
- `crates/server/src/http/events.rs` et `http/mod.rs` : la route `POST /api/events`, classe `create`.
- `crates/server/src/http/errors.rs` : les 11 codes et le champ `fields`.
- `crates/server/src/http/form_token.rs` : `FormJson` qui rend `form.malformed`.
- `crates/server/src/http/dto.rs` : `CreateEvent` et `CreatedEvent`.
- `crates/server/tests/events_create.rs` : 15 tests HTTP, qui couvrent la matrice, le quota, la migration et les journaux.
- `web/src/api/{client,events,types}.ts` : `postJson`, `fields`, le jeton et les nouveaux essais.
- `web/src/pages/{NewEvent,LinksCreated,OrganizerPending}.tsx` : les écrans.
- `web/src/components/{CopyChip,Stepper}.tsx` : les composants.
- `web/src/lib/time.ts` : la conversion de l'heure locale en UTC.
- `web/src/locales/{fr,en}.json` : les textes.

**Relecture :** 32 remarques : 18 corrigées, réparties en 10 correctifs, et 14 écartées, chacune avec sa raison dans le journal ci-dessus. Rien n'est reporté.

**Nouvelle relecture recommandée :** oui. Neuf entrées de gravité moyenne ont été corrigées, toutes dans la gestion d'état du formulaire `/new` : bornes, erreurs, portée et entiers. Le risque non vérifié est une interaction entre ces correctifs, par exemple un effacement d'erreur qui masquerait une erreur encore valide.

**Vérification :**
- fmt et clippy `-D warnings` passent, ainsi que `REQUIRE_WEB_DIST=1 cargo test`, sans aucun échec.
- `list-error-codes` : 25 codes, présents dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (116 tests). Le budget est respecté : 89,9 sur 150 ko de JS.
- Aucune adresse IP réelle dans le diff.

**Risques résiduels :**
- La mise en page sur un seul écran à 360 px et la création en moins de 60 s n'ont pas été vérifiées sur un vrai téléphone. La ligne de créneau est la plus serrée.
- La migration `0002` deviendra immuable une fois poussée : il faut la relire avant le push.
- Les liens `/e` et `/j` mènent à l'accueil jusqu'à la story 2-2.
