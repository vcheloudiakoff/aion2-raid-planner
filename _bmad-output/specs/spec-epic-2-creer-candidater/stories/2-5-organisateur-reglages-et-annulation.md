---
title: 'Organisateur : réglages et annulation'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '7a0837de279726df7fc9061df5428ec5dc90ba0f'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/commandes-et-droits.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-4-organisateur-trier-les-candidatures.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred:
  - summary: >-
      Les tests de journaux de `events_create.rs`, `organizer.rs`, `participant.rs` et `http/abuse_tests.rs` gardent la course d'enregistrement des callsites `tracing` (échec environ 1 fois sur 30), et le writer `Captured` est recopié dans cinq fichiers.
    evidence: |-
      Le préchauffage sans abonné laisse un autre fil enregistrer les callsites en premier et couper leur sortie ; corrigé seulement dans `candidature_submit.rs` et `organizer_settings.rs`. À regrouper dans `tests/common` avec le préchauffage sous abonné muet.
    location: >-
      crates/server/tests/
    severity: medium
---

<intent-contract>

## Intent

**Problem:** l'onglet Réglages de `/o` est en lecture seule. L'organisateur ne peut ni corriger sa note, son lien Discord, sa visibilité ou ses bornes, ni ajouter ou supprimer un créneau, ni annuler son raid (SPEC CAP-5, partie réglages).

**Approach:** quatre commandes organisateur en classe `write`, qui suivent la mutabilité d'AD-4 : réglages, ajout de créneau, suppression de créneau, annulation. L'onglet Réglages devient un formulaire qui garde le brouillon en cours, et l'annulation passe par une confirmation destructive. Cette story règle aussi le report de 2-2 : la course entre une suppression de créneau et une candidature.

## Boundaries & Constraints

**Always:**
- **Routes** `[ASSUMPTION]`, toutes sous `OrganizerLink`, dans le groupe `write`, corps lus par `FormJson` sans jeton, réponse 200 avec `OrganizerEvent`, aucun id dans l'URL :
  - `POST /api/organizer/settings` (`update_event_settings`) ;
  - `POST /api/organizer/slots`, corps `{startsAt, durationMin?}` (`add_slot`) ;
  - `POST /api/organizer/slot-removal`, corps `{slotId}` (`remove_slot`) ;
  - `POST /api/organizer/cancellation`, corps `{}` (`cancel_event`).
- **Mutabilité** (AD-4), dans une fonction pure `domain::event::event_may(status, action)` couverte par un test exhaustif. L'ordre des contrôles est le suivant :
  1. Un événement `cancelled` refuse toute écriture, y compris une seconde annulation : 409 `event.closed`.
  2. Les réglages sont permis en `open` comme en `locked`.
  3. L'ajout et la suppression de créneau ne sont permis qu'en `open`, sinon 409 `event.locked`.
  4. L'annulation est permise depuis `open` et `locked`.
- **Réglages :** le corps remplace tout : `{instance, region, factionScope, note, discordUrl?, visibility, minCp?, classBounds}`. Un `discordUrl` absent ou vide et un `minCp` absent effacent la valeur.
  - **Champs figés :** dès qu'une ligne `candidatures` existe pour l'événement (tous statuts confondus), un `instance`, un `region` ou un `factionScope` **différent** de la valeur actuelle donne 409 `event.field_immutable` (nouveau code), avec `fields` sur le chemin concerné. Ce refus passe avant toute erreur 400. Renvoyer la valeur inchangée passe toujours.
  - **Validation :** les règles de 2-1 sont réutilisées, mais « existe et `enabled` » ne s'applique qu'à une valeur changée (AD-6).
    - `note` et `discordUrl` : fonctions de `domain::validate`.
    - `minCp` : `cp`.
    - `factionScope` : il doit être compatible avec l'instance résultante.
  - **Bornes :** seules les lignes `class_bounds` existantes de l'événement sont mises à jour. Une classe sans ligne donne `reference.unknown` sur `classBounds.<id>`. Pour chaque ligne, `0 ≤ min ≤ max ≤ parties × party_size` de l'instance résultante.
- **Ajout de créneau :**
  - Règles extraites de `validate_new_event` dans un helper partagé :
    - `startsAt > now`, sinon `slot.in_past` ;
    - pas le même `startsAt` qu'un créneau existant, sinon `slot.duplicate` ;
    - `durationMin` par défaut celle de l'instance, entre 15 et 720, sinon `slot.duration_out_of_range` ;
    - au plus 14 créneaux, passés compris, sinon `slot.count_out_of_range`.
  - L'id du créneau vient d'`IdGenerator`.
- **Suppression de créneau :**
  - Un créneau inconnu ou d'un autre événement donne 404 `slot.not_found` (nouveau code).
  - Supprimer le dernier créneau restant donne 400 `slot.count_out_of_range`.
  - Un créneau passé peut être supprimé.
  - La suppression efface ses `availabilities` par cascade. Les statuts des candidatures ne changent jamais.
  - Une candidature restée sans disponibilité est conservée.
- **Annulation :** `status = cancelled` et `cancelled_at = clock.now()`. Aucune autre ligne ne change, et les liens restent valides.
- **Commandes** (modèle `set_candidature_status`) : transaction sur `db.write`, relecture de l'agrégat, règle pure, écriture, relecture de `organizer_view::load` dans la transaction, commit.
- **Report de 2-2 :** dans la transaction de `submit_candidature`, l'événement et ses créneaux sont relus, puis `validate_candidature` est rejoué sur ce contexte frais. Un créneau supprimé entre la lecture et l'écriture donne alors 400 `candidature.slot_unknown`, et un changement de région ou de portée est vu. Le 500 de clé étrangère disparaît.
- **Codes et textes :**
  - nouveaux codes `event.field_immutable` (409) et `slot.not_found` (404), en fr et en en ;
  - `errors.event.locked` reformulé de façon neutre, parce qu'il sert désormais à l'organisateur.
- **Onglet Réglages** (`components/EventSettings.tsx`) :
  - **Bloc « Le raid »** : instance, région et portée sont modifiables tant qu'aucune candidature n'existe. Ensuite, ils s'affichent en lecture avec « Fixés depuis la première candidature. »
  - **Bloc des réglages toujours modifiables** : note (compteur sur 280), lien Discord, case « Lister publiquement », CP minimum, bornes. Les bornes sont des Stepper qui parcourent `view.classBounds`.
  - **Enregistrement** : un bouton principal unique « Enregistrer », désactivé tant que rien n'a changé ou que le formulaire est invalide, avec la raison affichée dessous et « Envoi… » pendant l'envoi. Les nombres sont des entiers stricts, comme dans 2-1.
  - **Brouillon** : il part de la vue et n'est jamais écrasé par le rafraîchissement tant qu'il a des changements non enregistrés. Il repart de la vue qu'une commande renvoie.
  - **Bloc « Créneaux »** (en `open` seulement) :
    - la liste des créneaux, en heure locale ;
    - un « × » par créneau ;
    - un ajout par date-heure native et durée, avec un bouton « Ajouter ».
    - Supprimer un créneau coché par au moins une candidature non retirée demande une `ConfirmSheet` : « N joueurs ont coché ce créneau : leur dispo sera effacée. », bouton « Supprimer le créneau ». Sinon, la suppression est directe.
    - En `locked`, le bloc affiche « Le raid est verrouillé : les créneaux ne bougent plus. »
  - **Annulation** : un bouton secondaire de danger « Annuler le raid », en bas, ouvre une `ConfirmSheet` : « Le raid sera annulé pour tout le monde. Ça ne se défait pas. » Les boutons sont « Annuler le raid » (danger) et « Garder le raid ».
  - **Raid annulé** : l'onglet est en lecture seule et n'a aucune action.
  - **Erreurs** :
    - `fields` est affiché sous les champs ;
    - sur un code d'état (`event.closed`, `event.locked`, `event.field_immutable`, `slot.not_found`), l'erreur s'affiche et la vue est relue ;
    - les commandes passent par le séquencement `issued`/`applied`/`busyRef` d'`OrganizerPage`.
  - **Rafraîchissement** : pas de quatrième copie de la boucle. `EventSettings` reçoit la vue et des callbacks d'`OrganizerPage`.
- **Locales :** tous les textes en fr et en, ton *Voice and Tone*.

**Never:**
- Ajouter une migration : `cancelled_at` et la cascade existent déjà.
- Modifier un créneau en place : on le supprime puis on en ajoute un autre.
- Ajouter ou supprimer des lignes `class_bounds`.
- Livrer « Dupliquer » ou « Copier mon lien » (story 2-6), le déverrouillage ou `slot.hasGroups` (epic 3), ou la purge (epic 5).
- Ajouter un code `event.cancelled` en doublon d'`event.closed`.
- Mettre un id ou un secret dans une URL d'API.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Réglages | `open`, note, Discord, `unlisted`, minCp, bornes changés | 200 ; valeurs écrites ; `discordUrl` vide → `NULL` | — |
| Réglages en `locked` | mêmes changements | 200 | — |
| Champ figé | une candidature `withdrawn` existe, `region` différente | rien n'est écrit | 409 `event.field_immutable`, `fields.region` |
| Champ figé inchangé | une candidature existe, mêmes `instance`/`region`/`factionScope` | 200 | — |
| Bornes invalides | `min > max`, ou classe sans ligne | rien n'est écrit | 400 `event.class_bounds_invalid` ou `reference.unknown` |
| Ajout | `open`, créneau futur | 200 ; créneau ajouté, durée de l'instance par défaut | passé → 400 `slot.in_past` ; doublon → 400 `slot.duplicate` ; 15ᵉ → 400 `slot.count_out_of_range` ; `locked` → 409 `event.locked` |
| Suppression | `open`, créneau coché par 2 candidatures | 200 ; créneau et ses `availabilities` supprimés ; statuts inchangés | inconnu ou autre événement → 404 `slot.not_found` ; dernier → 400 `slot.count_out_of_range` ; `locked` → 409 `event.locked` |
| Annulation | `open` ou `locked` | 200 ; `cancelled`, `cancelled_at` = now | déjà annulé → 409 `event.closed` |
| Écriture sur annulé | réglages, ajout ou suppression | rien n'est écrit | 409 `event.closed` |
| Course de 2-2 | créneau supprimé après la lecture hors transaction de `submit_candidature` | rien n'est écrit | 400 `candidature.slot_unknown`, jamais 500 |

</intent-contract>

## Code Map

- `crates/domain/src/event.rs:16-20,87-119,169-186,215-245,324` -- bornes `SLOTS_*` et `DURATION_*`, `EventStatus`, `NewSlot`, `NewBound`, `EventError`, `validate_new_event` (boucle par créneau à extraire) et `default_bound`.
- `crates/domain/src/candidature.rs:229-312,402` -- `validate`, et `organizer_may`, modèle de `event_may`.
- `crates/server/src/infra/events.rs:15,66-123,198,227` -- `insert`, `SlotRow`, `EventSnapshot`, `load`, `status_and_candidature_count` (tous statuts) et `discord_url`. À ajouter : la mise à jour des réglages, la mise à jour des bornes, l'insertion et la suppression de créneau, l'annulation.
- `crates/server/migrations/0002_events.sql:18,60` -- `cancelled_at`, et la cascade des `availabilities` vers `slots`. Lecture seule.
- `crates/server/src/app/commands/set_candidature_status.rs:74-100` -- modèle de commande organisateur ; `queries/organizer_view.rs:45` fournit `load(&mut tx, …)`.
- `crates/server/src/app/commands/submit_candidature.rs:51-57,127-137` -- validation hors transaction, puis relecture partielle : c'est le report de 2-2. `update_candidature.rs:90-103` sert de modèle de revalidation dans la transaction.
- `crates/server/src/http/organizer.rs:40` et `http/mod.rs:79-95` -- `respond()` et le groupe `write`.
- `crates/server/src/http/dto.rs:212,232,433,440` -- `CreateEvent`, `NewSlot` (réutilisable pour l'ajout), `Withdrawal {}` (modèle de corps vide) et `OrganizerEvent`.
- `crates/server/src/http/errors.rs:53-93` -- `event.closed`, `event.locked` et `slot.*` existent déjà ; `event.field_immutable` et `slot.not_found` manquent.
- `crates/server/tests/common/candidatures.rs:66-105,210` et `tests/organizer.rs:23,464,555,595` -- aides `create_with`, `candidate`, `sql`, `set_event_status`, et modèles des tests annulé, quota et journaux.
- `web/src/pages/OrganizerPage.tsx:26-28,98-119,161-214,287,316-331` -- `STATE_CODES`, `TABS`, séquencement, `decide`, bandeau annulé et Réglages actuels.
- `web/src/pages/NewEvent.tsx:16-17,35-49,126-157,311-503` -- `NOTE_MAX`, `SLOTS_MAX`, `errorText`, `isIntOrEmpty`, `defaultBound`, `reason`, `errorLine`, `clearField`, lignes de créneaux, note, Discord, liste et Stepper. Les éditeurs communs sont à extraire plutôt qu'à copier.
- `web/src/components/{ConfirmSheet,Stepper,Sheet}.tsx` et `lib/time.ts:8,20,36` -- confirmation, Stepper, `localInputToUtc`, `timeZoneLabel` et `formatSlot`.
- `web/src/api/organizer.ts` et `test/fixtures.ts:139` -- `postJson` avec `linkSecret`, et la fixture `organizerEvent`.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/event.rs` -- `event_may`, le helper de créneau partagé avec `validate_new_event` et `validate_settings` (changements seulement, champs figés), avec leurs tests unitaires.
- `crates/server/src/infra/events.rs` -- écritures des réglages, des bornes, des créneaux et de l'annulation.
- `crates/server/src/app/commands/{update_event_settings,add_slot,remove_slot,cancel_event}.rs` -- les quatre commandes.
- `crates/server/src/app/commands/submit_candidature.rs` -- revalidation dans la transaction (report de 2-2).
- `crates/server/src/http/organizer.rs`, `http/mod.rs`, `http/dto.rs` et `http/errors.rs` -- routes, DTO d'entrée et 2 codes.
- `crates/server/tests/organizer_settings.rs`, plus un test de course dans `tests/candidature_submit.rs` -- chaque ligne de la matrice, le quota `write` et des journaux sans secret.
- `web/src/components/EventSettings.tsx`, `web/src/components/{SlotEditor,BoundsEditor}.tsx` (ou des éditeurs extraits de `NewEvent`), `pages/OrganizerPage.tsx` et `api/organizer.ts` -- l'onglet Réglages.
- `web/src/locales/fr.json` et `en.json` -- textes, codes et `errors.event.locked` neutre.
- Tests web, dans `web/src/**/*.test.tsx` :
  - brouillon non écrasé par le rafraîchissement ;
  - champs figés en lecture dès qu'une candidature existe ;
  - « Enregistrer » désactivé sans changement ;
  - entiers stricts ;
  - ajout et suppression de créneau ;
  - confirmation quand le créneau a des cochés ;
  - créneaux masqués en `locked` ;
  - annulation confirmée, avec « Garder le raid » qui referme ;
  - lecture seule une fois le raid annulé ;
  - code d'état qui déclenche une relecture ;
  - `NewEvent` sans régression.

**Acceptance Criteria:**
- Étant donné un raid annulé depuis `/o`, quand on lit `/api/events/{id}` et `/api/participant`, alors les deux renvoient `status: cancelled` et refusent toute écriture avec `event.closed`.
- Étant donné les locales, quand `list-error-codes` est vérifié, alors `event.field_immutable` et `slot.not_found` sont traduits.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 35 findings — high 0, medium 9, low 25, false 1, maybe-false 0
- findings:
  - `[low]` `[reject]` Interface et serveur jamais testés ensemble — chaque côté est testé sur son contrat (corps envoyé contre JSON reçu), comme dans les stories précédentes.
  - `[low]` `[patch]` `/e` jamais relu après un changement — relecture de `GET /api/events/{id}` après une suppression de créneau, et test du `noindex` après passage en `unlisted`.
  - `[low]` `[patch]` Le commentaire du test de suppression annonce plus qu'il ne vérifie — même correctif, qui ajoute une vraie lecture publique.
  - `[low]` `[reject]` `/p` non relu après un changement de Discord ou une suppression de créneau — la lecture participant compose les mêmes requêtes (`discord_url` et `availabilities`), déjà testées en 2-3.
  - `[low]` `[reject]` Champs figés décidés de chaque côté — `organizer_view` liste tous les statuts, donc les deux règles coïncident par construction.
  - `[false]` `[reject]` Refus inatteignables depuis l'onglet — c'est voulu : l'interface ne propose pas un geste que le serveur refuserait, et le serveur reste la garde.
  - `[low]` `[reject]` Écrans hors de l'onglet touchés sans nouveau test — le remaniement de `/new` est couvert par sa suite existante, qui passe sans changement.
  - `[low]` `[reject]` « Enregistrer » désactivé sans changement, contre « seulement si invalide » dans EXPERIENCE — choix de la spec : un enregistrement vide ne sert à rien et la raison est affichée.
  - `[low]` `[patch]` Ajout d'un créneau sur une instance retirée non testé — test HTTP `add_a_slot_on_a_retired_instance_takes_its_duration`.
  - `[low]` `[patch]` Filtre des instances désactivées non testé — fixture `i_off` et tests du cas courant et du cas normal.
  - `[low]` `[patch]` Messages d'erreur d'ajout et de suppression non testés — trois tests 429.
  - `[medium]` `[patch]` Une décision est perdue sans message pendant une commande Réglages — les boutons de la feuille sont désactivés (`otherPending`) ; testé.
  - `[medium]` `[patch]` La saisie faite pendant l'enregistrement est perdue — le `fieldset` est désactivé pendant l'envoi ; testé.
  - `[medium]` `[patch]` Un changement d'instance refusé garde des bornes rétrécies — quand les champs sont figés, les bornes reviennent à celles de la vue ; testé.
  - `[low]` `[reject]` Les bornes ne font que rétrécir au changement d'instance — même comportement que `/new`, et les Stepper permettent de remonter.
  - `[low]` `[patch]` La durée suggérée vient de l'instance non enregistrée — elle vient désormais de l'instance stockée.
  - `[low]` `[patch]` Le plafond de repli bouge avec le brouillon — il est calculé depuis `view.classBounds`.
  - `[medium]` `[patch]` Une saisie illisible efface le CP minimum stocké — `validity.badInput` est traité comme invalide ; testé.
  - `[low]` `[reject]` Durées des créneaux non revues quand l'instance change — chaque créneau garde sa durée explicite ; changer d'instance avant la première candidature reste rare.
  - `[low]` `[patch]` Le test de suppression vérifie la mauvaise réponse — même correctif que la relecture publique.
  - `[low]` `[patch]` Le test exhaustif d'`event_may` recopie le code — remplacé par une table littérale de 12 lignes.
  - `[medium]` `[defer]` Le correctif du test de journaux instable n'est appliqué qu'à deux fichiers, et `Captured` est dupliqué — l'instabilité existait avant cette story dans `events_create.rs`, `organizer.rs`, `participant.rs` et `http/abuse_tests.rs`.
  - `[low]` `[reject]` Les mêmes lignes sont lues deux fois dans la transaction — coût négligeable.
  - `[low]` `[reject]` Durée absente signalée comme hors bornes — seulement quand l'instance manque de la référence, un cas rare ; le message reste vrai.
  - `[low]` `[patch]` Tests HTTP manquants (instance retirée, effet d'`unlisted`) — mêmes correctifs que plus haut.
  - `[low]` `[patch]` En anglais, « locked » sert à deux choses — remplacé par « Fixed since… » et « have been fixed since… ».
  - `[low]` `[reject]` Textes de locale dupliqués — c'est cosmétique.
  - `[low]` `[reject]` `EventWriteError` s'ajoute aux cinq `WriteError` — c'est un motif hérité ; le factoriser relève d'un remaniement.
  - `[low]` `[patch]` Instance enregistrée absente de la référence et non proposée — option synthétique ; testé.
  - `[low]` `[patch]` Région enregistrée absente de la référence — même correctif.
  - `[low]` `[patch]` Durée suggérée par l'instance du brouillon (Edge Case Hunter) — même correctif que la durée suggérée.
  - `[medium]` `[patch]` Bornes rétrécies gardées après le gel des champs (Edge Case Hunter) — même correctif que les bornes.
  - `[medium]` `[patch]` `badInput` sur le CP minimum (Edge Case Hunter) — même correctif que `badInput`.
  - `[medium]` `[patch]` `badInput` sur la durée (Edge Case Hunter) — même correctif que `badInput`.
  - `[medium]` `[patch]` Décision perdue pendant une commande Réglages (Edge Case Hunter) — même correctif que `otherPending`.

## Design Notes

**Pas de code `event.cancelled` :** `event.closed` signifie déjà « événement annulé » partout. Deux codes pour le même état iraient contre AD-5, et `commandes-et-droits.md` ne donne ses codes qu'à titre d'exemple.

**« Première candidature » = une ligne, quel que soit son statut :** le serveur de toute candidature, même retirée, a été validé contre cette région et cette portée. Les changer rendrait ces lignes incohérentes.

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

**Résumé :** quatre commandes organisateur, toutes en classe `write`, sans jeton, et renvoyant `OrganizerEvent` :
- `POST /api/organizer/settings` remplace tous les réglages ;
- `POST /api/organizer/slots` ajoute un créneau ;
- `POST /api/organizer/slot-removal` supprime un créneau ;
- `POST /api/organizer/cancellation` annule le raid.

La mutabilité d'AD-4 est portée par `event_may` :
- les réglages sont modifiables en `open` comme en `locked` ;
- les créneaux ne bougent qu'en `open` ;
- un raid `cancelled` refuse tout, avec `event.closed`.

Instance, région et portée se figent dès la première candidature, quel que soit son statut, avec le nouveau code `event.field_immutable`. Un créneau inconnu donne le nouveau code `slot.not_found`. Supprimer un créneau efface en cascade ses disponibilités, sans changer les statuts. Le dernier créneau ne peut pas être supprimé.

Le report de 2-2 est réglé : `submit_candidature` revalide dans sa transaction, si bien qu'un créneau supprimé entre-temps donne un 400, jamais un 500.

L'onglet Réglages (`EventSettings`) contient :
- les blocs « Le raid » (figés après la première candidature), réglages et créneaux ;
- une annulation confirmée par une `ConfirmSheet` (« Annuler le raid » / « Garder le raid ») ;
- un brouillon que le rafraîchissement n'écrase pas ;
- des éditeurs extraits de `/new` et partagés.

**Fichiers :**
- `crates/domain/src/event.rs` : `event_may`, `validate_settings`, `validate_added_slot` et le helper de créneau.
- `crates/server/src/app/commands/{update_event_settings,add_slot,remove_slot,cancel_event}.rs` et `commands/mod.rs`.
- `crates/server/src/app/commands/submit_candidature.rs` : revalidation dans la transaction.
- `crates/server/src/infra/events.rs` : les écritures.
- `crates/server/src/http/{organizer,mod,dto,errors}.rs` : les routes et les 2 codes.
- Tests serveur : `crates/server/tests/organizer_settings.rs` (18 tests), et le test de course dans `candidature_submit.rs`.
- `web/src/components/{EventSettings,EventFields,SlotEditor,BoundsEditor}.tsx`, `lib/form.tsx`, `pages/{OrganizerPage,NewEvent}.tsx`, `components/CandidatureSheet.tsx`, `api/organizer.ts` et les locales (les clés `new.*` partagées passent en `editor.*`).

**Relecture :** 35 remarques.
- 23 corrigées, réparties en 15 correctifs.
- 1 reportée : l'instabilité des tests de journaux, déjà présente avant cette story.
- 11 écartées, dont 1 fausse, chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** oui. Cinq entrées de gravité moyenne ont été corrigées. Le risque non vérifié est le gel du `fieldset` et le drapeau `badInput` dans un vrai navigateur : happy-dom ne reproduit pas toutes les règles des champs numériques.

**Vérification :**
- fmt et clippy `-D warnings` passent ; `REQUIRE_WEB_DIST=1 cargo test` : 282 tests, 0 échec.
- `list-error-codes` : 41 codes, présents dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (247 tests). Le budget est respecté : 104,2 sur 150 ko de JS.
- Aucune adresse IP dans le diff.

**Risques résiduels :**
- Deux choix sont à confirmer :
  - les champs figés comptent une candidature retirée comme une « première candidature » ;
  - une candidature dont le seul créneau est supprimé reste, sans disponibilité.
- Le rendu n'a pas été vérifié sur téléphone.
