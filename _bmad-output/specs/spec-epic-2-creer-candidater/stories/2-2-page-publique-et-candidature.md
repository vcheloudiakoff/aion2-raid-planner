---
title: 'Page publique et candidature'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '0018cb682ba59f8d5a037976b7095544b4c0f0e7'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/commandes-et-droits.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-1-creer-un-evenement-et-recevoir-ses-liens.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred:
  - summary: >-
      Un créneau supprimé entre la lecture de l'événement et l'écriture de la candidature fait échouer la clé étrangère de `availabilities` avec un 500.
    evidence: |-
      `submit_candidature` valide les créneaux sur un instantané lu hors transaction et ne relit que le statut et le compte. Inatteignable tant que `remove_slot` (story 2-5) n'existe pas ; 2-5 doit relire les créneaux dans la transaction ou traduire la violation en `candidature.slot_unknown`.
    location: >-
      crates/server/src/app/commands/submit_candidature.rs
    severity: medium
---

<intent-contract>

## Intent

**Problem:** un raid créé n'a encore aucune page. Les liens `/e` et `/j` mènent à l'accueil et aucun joueur ne peut candidater (SPEC CAP-2, CAP-3).

**Approach:**
- **Lecture `public_event` :** un DTO `PublicEvent` sans aucune donnée nominative, avec les comptes calculés côté serveur.
- **Commande `submit_candidature` :** une transaction, sur le modèle de `create_event`.
- **Pages `/e/:id` et `/j/:secret` :** elles partagent le même écran, résumé puis formulaire sur un seul écran.
- **Fin du parcours :** le formulaire mène au lien personnel `/p/:secret`. La page complète de `/p` est la story 2-3.

## Boundaries & Constraints

**Always:**

- **Routes d'API** (chemins fixés ici `[ASSUMPTION]`, le secret toujours dans `X-Link-Secret`) :
  - `GET /api/events/{event_id}` : public, classe `read` ;
  - `GET /api/join` : droit `join`, classe `read` ;
  - `POST /api/events/{event_id}/candidatures` : public, origine `public`, statut `pending`, classe `candidature` ;
  - `POST /api/join/candidatures` : droit `join`, origine `direct`, statut `accepted`, classe `candidature`.
- **Extracteur `JoinLink` :** les routes `/api/join*` n'ont pas d'`event_id` dans le chemin.
  - Il vit dans `http/links.rs`, résout le secret par `links::find` et exige `Right::Join`.
  - L'événement vient de la ligne de lien.
  - Erreurs : `auth.missing`, `auth.not_found`, `auth.wrong_right`, comme `LinkSecret`.
  - La doc de module de `links.rs` le documente comme la seule exception à la règle de portée par chemin.
- **`PublicEvent`** (`http::dto`, recopié à la main dans `types.ts`, camelCase) :
  - champs : `id`, `instance`, `region`, `factionScope`, `visibility`, `status`, `minCp?`, `note`, `slots[{id, startsAt, durationMin}]` triés par `startsAt` ;
  - `countsByClass` : `{classId: n}` pour chaque ligne `class_bounds` de l'événement, zéros compris ;
  - `countsByRole` : `{roleId: n}` ;
  - `missingRoles` : `[{role, missing}]`.
- **Ce que `PublicEvent` ne contient jamais :** pseudo, serveur de candidat, id de candidature, `discordUrl`, `adminUnlistedAt`, `createdAt`. Un test sérialise la réponse et vérifie que ces clés et un pseudo inséré n'y figurent pas.
- **Règle de comptage `[ASSUMPTION]`** :
  - Comptés : les candidatures `pending` et `accepted`. `rejected` et `withdrawn` ne le sont pas, pour que la page ne trahisse aucune décision de l'organisateur.
  - Rôle : celui de la classe, lu dans la référence. Une classe inconnue de la référence compte sous son propre id (AD-6).
  - Manque d'un rôle : la somme des `min_count` des classes de ce rôle, moins les candidatures comptées de ce rôle.
  - `missingRoles` ne garde que les manques strictement positifs, triés par id de rôle.
- **Événement absent :** `GET /api/events/{id}` sur un id inconnu donne 404 `event.not_found`. Un événement `cancelled` reste lisible, `status` vaut alors `cancelled`.
- **En-têtes HTML de `/e/{id}` :**
  - la page porte `X-Robots-Tag: noindex` si l'événement est `unlisted`, si `admin_unlisted_at` est posé, ou s'il est inconnu ;
  - la route `/e/{id}` sert `index.html` après une lecture sur `db.read` ;
  - aucune page n'a de métadonnée `og:`.
- **Entrée de `submit_candidature`** (`FormEnvelope<SubmitCandidature>`, classe de jeton `candidature`) :
  - `pseudo`, `server`, `class`, `cp` ;
  - `availabilities[{slotId, preferred}]`, où une entrée vaut un créneau coché.
- **Validation** (pure, dans `domain::candidature`) : toutes les erreurs sont collectées, dans l'ordre des champs, avec des chemins camelCase.
  - `pseudo` : `validate::pseudo` → `candidature.pseudo_invalid`.
  - `server` : il existe et est `enabled`, sinon `reference.unknown`. Il appartient à la région de l'événement, sinon `candidature.server_out_of_region`. Sa faction est dans `faction_scope`, sinon `candidature.faction_out_of_scope`.
  - `class` : elle existe, est `enabled` et a une ligne `class_bounds` dans l'événement, sinon `reference.unknown`.
  - `cp` : `validate::cp` → `candidature.cp_out_of_range`. `min_cp` n'est qu'un signal et ne refuse rien.
  - `availabilities` :
    - au moins une entrée, sinon `candidature.no_availability` ;
    - chaque `slotId` appartient à l'événement, sinon `candidature.slot_unknown` ;
    - le créneau n'est pas passé, sinon `slot.in_past` ;
    - pas de doublon, sinon `candidature.slot_unknown`.
  - `candidature.preference_not_available` (400) : il est renvoyé pour un `preferred` posé sur un créneau absent de la liste. Il est inatteignable par construction du DTO, mais il est enregistré et traduit.
- **Erreurs d'état** (409, vérifiées dans la transaction) :
  - `event.closed` : l'événement est `cancelled`. Une candidature en `locked` reste acceptée (AD-4).
  - `candidature.limit_reached` : 200 lignes `candidatures` existent déjà pour l'événement, tous statuts confondus.
- **`Problem::fields`** devient générique (`E: Into<Code>`) pour servir aux erreurs d'événement comme de candidature.
- **Ordre dans la commande** (celui de `create_event`) :
  1. `check` du jeton avec `FormClass::Candidature` ;
  2. lecture de l'événement, pour valider contre ses créneaux et sa région ;
  3. validation ;
  4. transaction sur `db.write` ;
  5. `breaker.check(Trip::Candidatures)`, puis relecture du statut et du compte ;
  6. insertions : `candidatures`, les `availabilities`, et un lien `participant` avec `candidature_id` ;
  7. commit ;
  8. `breaker.record(Trip::Candidatures)`.
- **Valeurs écrites :** `created_at` et `updated_at` viennent de `clock.now()`. Aucun secret ni pseudo n'apparaît dans les journaux.
- **`CreatedCandidature`** : `{eventId, candidatureId, participantSecret, status, links: {participant: "/p/<s>"}}`, réponse 201.
- **Écran `/e/:id` et `/j/:secret`** (un seul composant, avec un mode) :
  - En tête, le résumé :
    - instance + date du premier créneau en titre ;
    - région, portée (point de faction 8 px), note ;
    - créneaux en heure locale, jour court (« mar. 21:00 »), fuseau indiqué une fois ;
    - une ligne de rôles, du type « 3 inscrits · manque 1 Soin » ;
    - CP minimum s'il existe.
  - Ensuite, le formulaire, dans cet ordre :
    1. pseudo ;
    2. serveur, groupé par faction et filtré par région et portée ;
    3. classe, limitée aux classes de l'événement ;
    4. CP, entier ;
    5. un bouton ✓ par créneau à venir, avec `aria-pressed`, et une ☆ qui marque la préférence, seulement sur un créneau coché ; plusieurs ☆ sont permises.
  - Après le choix, la faction et le rôle s'affichent en lecture seule.
  - Les libellés de jeu viennent de la référence.
  - Le mode `/j` ajoute la ligne « Lien direct : ta candidature est acceptée d'office. »
- **Comportement du formulaire :**
  - Bouton unique « Candidater » : la raison de désactivation est affichée sous le bouton, « Envoi… » pendant l'envoi. Un CP sous `minCp` affiche une simple indication.
  - Jeton, nouvel essai, `fields` sous les champs et saisie conservée : même politique que `/new`. La boucle de nouvel essai de `api/events.ts` est rendue générique (URL, classe, options).
  - Le formulaire reprend `pseudo`, `server` et `class` depuis `prefs.candidate` (`lib/storage`), qui est écrit après un succès. Le CP n'est jamais retenu.
  - Une erreur `limit.paused` affiche `errors.limit.paused`.
  - Rafraîchissement silencieux du résumé toutes les 10 s, sans toucher la saisie.
- **États de la page :**
  - `cancelled` : bandeau « Raid annulé » et formulaire masqué, sur `/e` comme sur `/j`.
  - id ou secret inconnu (`event.not_found`, `auth.not_found`) : le message unique « Ce lien ne mène à rien. Il a peut-être expiré. » ; `errors.event.not_found` et `errors.auth.not_found` ont le même texte.
  - Le secret de `/j` n'apparaît jamais dans `document.title`.
- **Après le succès :**
  1. `saveParticipantLink(eventId, candidatureId, secret)` est appelé ;
  2. le navigateur va vers `/p/:secret`, avec la réponse passée dans l'état de navigation.
- **Page `/p/:secret` :** page provisoire `ParticipantPending`, remplacée en 2-3.
  - En-tête « Ton lien personnel », puis une `CopyChip` du lien complet, un bouton « Partager » si Web Share existe et l'avertissement « Garde ce lien : c'est le seul moyen de revoir ta candidature. ».
  - Si l'état de navigation est présent, elle affiche ensuite le statut : « En attente de l'organisateur » pour `pending`, « Acceptée » pour `accepted`.
  - Si `saveParticipantLink` a renvoyé false : « Ce navigateur ne gardera pas le lien : copie-le maintenant. »
- **Locales :** tous les textes et les nouveaux `errors.<code>` sont en fr et en en. Ton *Voice and Tone*.

**Never:**
- Toucher aux migrations 0001, 0002 et 9999 : ce modèle suffit.
- Stocker la faction ou le rôle.
- Exposer une liste nominative ou un compte par serveur.
- Ajouter un compte, un email ou un contact.
- Livrer la vue organisateur ou la modification et le retrait d'une candidature (stories 2-3, 2-4).
- Mettre l'id d'une candidature ou un secret dans une URL d'API.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Lecture publique | event avec 1 `pending` Clerc, 1 `rejected` Clerc, min Clerc = 2 | 200 ; `countsByClass.cleric=1`, `countsByRole.healer=1`, `missingRoles=[{role:"healer", missing:1}]` ; aucun pseudo dans le corps | — |
| Lecture inconnue | id absent | — | 404 `event.not_found` |
| Lecture `/j` | secret join valide | même `PublicEvent` | secret organisateur → 403 `auth.wrong_right` ; inconnu → 404 `auth.not_found` ; absent → 401 `auth.missing` |
| Candidature `/e` | jeton valide, serveur de la région, 2 créneaux dont 2 préférés | 201 ; `pending`, `origin=public`, 2 `availabilities` `preferred=1`, un lien `participant` | — |
| Candidature `/j` | même corps par secret join | 201 ; `accepted`, `origin=direct` | — |
| Serveur hors région ou faction | serveur d'une autre région ; faction hors portée | rien n'est écrit | 400 `candidature.server_out_of_region` ou `candidature.faction_out_of_scope` avec `fields.server` |
| Créneaux | aucun ; `slotId` étranger ; créneau passé | rien n'est écrit | 400 `candidature.no_availability`, `candidature.slot_unknown`, `slot.in_past` |
| Pseudo et CP | pseudo de 17 caractères ; CP 1 000 000 | rien n'est écrit | 400, `fields.pseudo` et `fields.cp` |
| Événement annulé | `status=cancelled` | rien n'est écrit | 409 `event.closed` |
| Plafond | 200 candidatures existantes | rien n'est écrit | 409 `candidature.limit_reached` |
| Disjoncteur | `MAX_CANDIDATURES_PER_HOUR` atteint | rien n'est écrit | 503 `limit.paused` + `Retry-After` |
| Page `unlisted` | GET `/e/<id>` HTML, event `unlisted` | `X-Robots-Tag: noindex` ; un event `listed` ne l'a pas | — |

</intent-contract>

## Code Map

- `crates/server/src/app/commands/create_event.rs:30-107` -- modèle de commande : jeton, référence, validation, `write()` avec `WriteError`, puis disjoncteur, insertions, commit et `record`.
- `crates/server/src/infra/events.rs:12` -- insertions de l'événement. On y ajoute la lecture (`load_public`) et les comptes ; les insertions de candidature vont dans le nouveau `infra/candidatures.rs`.
- `crates/server/migrations/0002_events.sql` -- tables `candidatures` et `availabilities`. Lecture seule.
- `crates/domain/src/event.rs:120-137` -- modèle `EventError`, `FieldErrors` et `validate_*`. Le nouveau module est `domain/src/candidature.rs`.
- `crates/domain/src/validate.rs:46,73` -- `pseudo` et `cp`.
- `crates/domain/src/reference.rs:28,70,97` -- `Faction`, `Server {region, faction, enabled}` et `Class {role}`. Pas d'aide de recherche : parcours par `id`.
- `crates/server/src/http/errors.rs:52-77,110,164` -- `codes!{}`, `From<EventError>` et `Problem::fields`, à rendre générique.
- `crates/server/src/http/dto.rs:211-294` -- `CreateEvent`, avec des `i64` pour que le dépassement devienne une erreur de champ, et `CreatedEvent`. Les DTO de reference sont aux lignes 58-98.
- `crates/server/src/http/events.rs` -- `post_event`. Les nouveaux handlers vont dans `http/candidatures.rs`.
- `crates/server/src/http/mod.rs:38-61` -- groupes `read` et `create`. Il faut ajouter un groupe `candidature` (`RouteClass::Candidature`) et mettre les GET dans `read`.
- `crates/server/src/http/links.rs:3-17,61-172` -- `LinkSecret<R>`, et `scope()`, qui refuse une route sans `event_id`. `JoinLink` réutilise `links::find` et la même cartographie d'erreurs.
- `crates/server/src/http/headers.rs:37-62` -- `is_secret_path` ne couvre pas `/e` ; `X-Robots-Tag` est seulement inséré. Le test `secret_paths` dit que `/e/abc` n'est pas noindex, donc à adapter.
- `crates/server/src/http/assets.rs` -- repli SPA sans base. La route HTML `/e/{id}` vient s'ajouter avant ce repli.
- `crates/server/src/http/limits.rs:31,175` -- `RouteClass::Candidature` et `Trip::Candidatures`. `config.rs:163,178` : `MAX_CANDIDATURES_PER_HOUR` (300) et `RATE_CANDIDATURE`.
- `crates/server/src/http/form_token.rs:40,133-165` -- `FormClass::Candidature`, `FormEnvelope` et `FormJson`.
- `crates/server/src/infra/links.rs:65-90` -- `LinkRow {candidature_id: Some}` pour `Participant`, `insert` et `find`.
- `crates/server/tests/common/mod.rs:91,129-182` -- `send_with` envoie des en-têtes, mais `post_json` ne le peut pas. Il faut une variante qui en accepte. `tests/events_create.rs:28-86` fournit les aides `token`, `nominal`, `assert_fields` et `setup`.
- `web/src/App.tsx:24-31` -- routes ; ajouter `/e/:id`, `/j/:secret` et `/p/:secret`.
- `web/src/api/client.ts:100-150` -- `linkSecret`, `getJson` et `postJson`. `api/events.ts:15-33` porte la boucle de nouvel essai, à rendre générique.
- `web/src/api/links.ts:94` -- `saveParticipantLink(eventId, candidatureId, secret): boolean`.
- `web/src/lib/time.ts` -- `localInputToUtc` et `timeZoneLabel`. Il faut un formateur jour + heure.
- `web/src/pages/NewEvent.tsx:35,61-67,126-200,509` -- modèles à reprendre : `errorText`, jeton prérécupéré, `errorLine` et `clearField`, raison sous le bouton, honeypot, `limit.paused`.
- `web/src/components/CopyChip.tsx` et `web/src/pages/LinksCreated.tsx` -- copie, partage et avertissement de stockage.
- `web/src/test/fixtures.ts` -- `servers: []`, à remplir pour les tests de candidature.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/candidature.rs` (nouveau) -- `NewCandidature`, `validate_candidature(&ReferenceSet, &EventContext, now, input)` et `CandidatureError`, avec des tests unitaires de chaque règle -- règles pures.
- `crates/domain/src/counts.rs` (ou dans `candidature.rs`) -- `public_counts(&ReferenceSet, bounds, candidatures)`, avec ses tests, dont le cas d'une classe inconnue -- règle de comptage testable.
- `crates/server/src/http/errors.rs` -- nouveaux codes : `event.not_found` (404), `event.closed` (409), `candidature.pseudo_invalid`, `candidature.cp_out_of_range`, `candidature.server_out_of_region`, `candidature.faction_out_of_scope`, `candidature.no_availability`, `candidature.slot_unknown`, `candidature.preference_not_available` (400) et `candidature.limit_reached` (409). `Problem::fields` devient générique.
- `crates/server/src/infra/events.rs` et `infra/candidatures.rs` -- lecture de l'événement, de ses créneaux, bornes et candidatures comptées, puis insertions de la candidature.
- `crates/server/src/app/queries/public_event.rs` et `app/commands/submit_candidature.rs` -- AD-5 et AD-16.
- `crates/server/src/http/links.rs` -- `JoinLink` et la doc de module.
- `crates/server/src/http/candidatures.rs`, `http/mod.rs`, `http/dto.rs`, `http/headers.rs` et la route HTML `/e/{id}` -- les 4 routes et le noindex conditionnel.
- `crates/server/tests/public_event.rs` et `tests/candidature_submit.rs`, plus une aide POST avec en-têtes -- chaque ligne de la matrice côté serveur, plus le test « aucune donnée nominative dans `PublicEvent` ».
- `web/src/api/{types,events,candidatures}.ts`, `lib/time.ts` -- DTO, boucle de nouvel essai générique, `getPublicEvent`, `getJoinEvent`, `submitCandidature`, formateur de date.
- `web/src/pages/EventPage.tsx`, `ParticipantPending.tsx`, `App.tsx`, `web/src/test/fixtures.ts` et `index.css` -- écrans d'*Always*.
- `web/src/locales/fr.json` et `en.json` -- textes et `errors.<code>`.
- `web/src/**/*.test.tsx` -- tests :
  - résumé et ligne de rôles ;
  - serveurs filtrés par région et portée ;
  - faction et rôle affichés après le choix ;
  - ☆ seulement sur un créneau coché, plusieurs ☆ envoyées ;
  - `prefs.candidate` repris et écrit ;
  - mode `/j` avec le secret dans l'en-tête et absent du titre ;
  - `cancelled` qui masque le formulaire ;
  - lien inconnu ;
  - `saveParticipantLink` puis navigation vers `/p` ;
  - avertissement quand le stockage est absent ;
  - routage dans `App.test.tsx`.

**Acceptance Criteria:**
- Étant donné un événement avec des candidatures de pseudos connus, quand on lit `GET /api/events/{id}` ou `GET /api/join`, alors le corps sérialisé ne contient aucun de ces pseudos ni aucune des clés `pseudo`, `server`, `candidatures`, `discordUrl` ou `candidatureId`.
- Étant donné une candidature réussie, quand on résout le `participantSecret` par `links::find`, alors on obtient `participant`, avec le bon `event_id` et le bon `candidature_id`, et aucun secret brut n'est stocké.
- Étant donné les locales, quand `list-error-codes` est vérifié, alors chaque nouveau code a sa traduction fr et en.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 32 findings — high 0, medium 12, low 15, false 5, maybe-false 0
- findings:
  - `[false]` `[reject]` `candidature.preference_not_available` n'est jamais émis, alors que CAP-3 demande un 400 pour une préférence sur un créneau non coché — une entrée vaut un créneau coché, donc une préférence sur un créneau non coché est impossible à exprimer. Le cas que vise CAP-3 ne peut pas se produire.
  - `[low]` `[reject]` Temps sur téléphone réel et délai de 10 s chez l'organisateur — mesures manuelles. La vue organisateur arrive en 2-4.
  - `[false]` `[reject]` `/p` n'est qu'une page provisoire — c'est le découpage : la page `/p` complète est la story 2-3.
  - `[low]` `[reject]` L'absence d'`og:` n'est testée que sur `/e` — `index.html` est le même pour toutes les routes de la SPA, donc le test de `/e` couvre le gabarit.
  - `[medium]` `[patch]` Classe du jeton au renouvellement non vérifiée — tests ajoutés : `renewToken("create")`, et une demande `class=candidature` avant le second POST.
  - `[low]` `[patch]` Plafond testé seulement côté refus — test ajouté : 201 à 199 lignes, puis 409.
  - `[medium]` `[patch]` Titre daté d'un créneau passé — le titre prend le premier créneau à venir (le dernier à défaut) et affiche l'année si elle diffère.
  - `[medium]` `[patch]` `/e/<id>/` et ses sous-chemins échappent au `noindex` — `spa_fallback` marque `noindex` tout chemin `/e/…` autre que la route exacte, et c'est testé.
  - `[low]` `[patch]` La route HTML interroge la base pour tout id de 22 caractères — le jeu de caractères base64url est vérifié avant la requête.
  - `[medium]` `[patch]` Titre sans le premier créneau à venir ni l'année (Blind Hunter) — même correctif que le titre.
  - `[medium]` `[patch]` Libellés de créneaux ambigus dans le résumé — la date est ajoutée à chaque créneau.
  - `[false]` `[reject]` Le formulaire propose des classes tirées de `countsByClass`, qui contiendrait des classes non bornées — une candidature ne passe que si sa classe a une ligne `class_bounds`, et ces lignes ne disparaissent pas. Les clés de `countsByClass` sont donc toujours des classes bornées.
  - `[medium]` `[defer]` Un créneau supprimé entre la lecture et l'écriture donne un 500 (clé étrangère) — impossible tant que `remove_slot` n'existe pas. À traiter avec 2-5.
  - `[medium]` `[patch]` Un événement annulé avec une erreur de champ répond 400 au lieu de 409 — le statut est vérifié avant la validation, et c'est testé.
  - `[low]` `[reject]` Lecture agrégée superflue dans la commande — au plus 200 lignes par la clé primaire, coût négligeable. La séparer ajouterait une variante de lecture.
  - `[medium]` `[patch]` Le rafraîchissement ne s'arrête jamais — il s'arrête sur les codes terminaux et sur `cancelled`, testé avec des minuteurs simulés.
  - `[low]` `[patch]` Erreur de créneau non rattachée au créneau — `aria-invalid`, une marque visible et un lien vers le texte d'erreur.
  - `[low]` `[reject]` Message trompeur pour un `slotId` en double — l'interface ne peut pas produire de doublon ; seul un client scripté y arrive.
  - `[low]` `[reject]` Trous de couverture de `/api/join` sur un événement annulé, code mort et `'listed'` écrit en dur — chemin de commande partagé avec `/e`, qui est testé ; la remarque est cosmétique.
  - `[low]` `[patch]` Hygiène : largeur des docs, ligne vide finale, alias `CreateDeps` — corrigé.
  - `[medium]` `[patch]` Annulé puis 400 (Edge Case Hunter) — même correctif que la vérification du statut.
  - `[medium]` `[patch]` Barre oblique finale sur `/e` (Edge Case Hunter) — même correctif que `spa_fallback`.
  - `[low]` `[reject]` Un événement listé puis annulé reste indexable — rien dans la spec ne le demande ; la page affiche « Raid annulé ».
  - `[medium]` `[patch]` Le rafraîchissement continue sur un lien mort — même correctif que l'arrêt du rafraîchissement.
  - `[low]` `[reject]` Course entre deux rafraîchissements lents — la donnée périmée est remplacée 10 s plus tard ; il faudrait une garde de séquence.
  - `[medium]` `[patch]` `auth.wrong_right` laisse un squelette sans fin — état terminal « Ce lien ne mène à rien », même correctif.
  - `[false]` `[reject]` Classes proposées tirées de `countsByClass` (Edge Case Hunter) — même réfutation que plus haut.
  - `[low]` `[patch]` Créneau en erreur sans `aria-invalid` — même correctif que le rattachement au créneau.
  - `[low]` `[reject]` Un créneau coché qui démarre pendant la saisie disparaît en silence — cas rare ; le serveur refuserait de toute façon ce créneau passé.
  - `[low]` `[reject]` Des milliers d'entrées `availabilities` factices — corps limité à 2 Mo et quota `candidature` ; le correctif ajouterait une garde.
  - `[false]` `[reject]` Code `preference_not_available` mort — même réfutation que la première ligne.
  - `[low]` `[reject]` Le jeton renouvelé n'est pas gardé pour la soumission suivante — cela coûte un aller-retour, et seulement après une expiration suivie d'une erreur de champ.

## Design Notes

**`JoinLink` sans `event_id` :** la page `/j/:secret` ne connaît pas l'événement, et AD-3 interdit de mettre le secret dans l'URL d'API. La portée vient donc de la ligne de lien elle-même. Seul le droit `join` peut se résoudre ainsi.

**Comptes sur `pending` + `accepted` :** la page publique sert à attirer des candidats. Le manque ne doit pas remonter quand l'organisateur refuse quelqu'un. Ce choix se change sans migration.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert, JS sous 150 KB.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.
- `cargo tree -e features --workspace --target all --locked | sh deploy/check-network-crates.sh` -- expected: OK.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh && sh deploy/tests/plan-deploy.test.sh` -- expected: OK.

## Auto Run Result

Status: done

**Résumé :** la lecture publique `public_event` passe par `GET /api/events/{id}`, ou par `GET /api/join` avec le secret direct dans l'en-tête. Le nouvel extracteur `JoinLink` résout ce secret sans `event_id` dans le chemin. Le DTO `PublicEvent` ne contient aucune donnée nominative, ce que vérifie un test. Ses comptes par classe et par rôle et ses rôles manquants sont calculés côté serveur, sur les candidatures `pending` et `accepted`.

La commande `submit_candidature` (classes de jeton et de limiteur `candidature`, disjoncteur `Trip::Candidatures`) enregistre la candidature, selon le lien utilisé :
- par `/e` : statut `pending`, `origin=public` ;
- par `/j` : statut `accepted`, `origin=direct` ;
- dans les deux cas : plusieurs préférences, plafond de 200 et un lien `participant`.

La page `/e/{id}` porte `noindex` si l'événement est non listé, masqué par l'administration ou inconnu, et sur tout sous-chemin.

Côté web, un seul écran `/e/:id` et `/j/:secret` montre le résumé, rafraîchi toutes les 10 s, puis le formulaire. Les pseudo, serveur et classe sont repris de `prefs.candidate`. Après l'envoi, le navigateur va vers la page provisoire `/p/:secret`.

**Fichiers :**
- `crates/domain/src/{candidature,counts}.rs` : règles pures de validation et de comptage.
- `crates/server/src/app/queries/public_event.rs` et `app/commands/submit_candidature.rs` : lecture et commande.
- `crates/server/src/infra/{events,candidatures}.rs` : lectures, insertions et indexabilité.
- `crates/server/src/http/{candidatures,events,links,mod,dto,errors}.rs` : les 4 routes API, la route HTML `/e/{id}` et `spa_fallback`, `JoinLink`, `PublicEvent`, 10 codes et `Problem::fields` générique.
- `crates/server/tests/{public_event,candidature_submit}.rs` : couverture de la matrice, quotas, journaux, données non nominatives et plafond.
- `web/src/api/{candidatures,events,types}.ts`, `lib/{time,storage}.ts` : API, nouvel essai générique `postForm`, formateurs et `prefs.candidate`.
- `web/src/pages/{EventPage,ParticipantPending}.tsx`, `App.tsx` et les locales : les écrans.

**Relecture :** 32 remarques.
- 19 corrigées, réparties en 11 correctifs.
- 1 reportée : la suppression de créneau en course, à traiter avec 2-5.
- 12 écartées, dont 5 fausses, chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** oui. Huit entrées de gravité moyenne ont été corrigées. Le risque non vérifié est l'arrêt du rafraîchissement : il ne doit pas couper la reprise après une simple panne réseau, qui n'est pas un code terminal.

**Vérification :**
- fmt et clippy `-D warnings` passent, ainsi que `REQUIRE_WEB_DIST=1 cargo test` : 204 tests, 0 échec.
- `list-error-codes` : 35 codes dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (145 tests). Le budget est respecté : 94,4 sur 150 ko de JS.
- Aucune adresse IP dans le diff.

**Risques résiduels :**
- La règle de comptage (`pending` + `accepted`) et le calcul du manque par la somme des minimums des classes d'un rôle sont des `[ASSUMPTION]`, à confirmer par Vincent.
- Le texte `errors.limit.paused` est devenu générique.
- La candidature en moins de 30 s n'a pas été vérifiée sur un vrai téléphone.
