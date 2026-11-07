---
title: 'Verrouiller et déverrouiller'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '0b4fe5e2f241baf56dbf148d5acc5b8a72af9366'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/placement-et-salle.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/stories/3-1-grille-des-disponibilites.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
warnings: [oversized]
deferred:
  - summary: >-
      Un placement dans un groupe en sommeil garde sa ligne `assignments` (UNIQUE candidature_id) : apply_placement (3-3) doit dire s'il conserve ou remplace ces lignes.
    evidence: |-
      Le groupe en sommeil garde ses placements (décision de Vincent) ; apply_placement « remplace en entier » le placement de l'événement. Sans règle explicite, re-placer ce joueur dans un groupe retenu heurte l'index unique ou efface le placement en sommeil.
    location: >-
      crates/server/migrations/0003_groups.sql
    severity: medium
---

<intent-contract>

## Intent

**Problem:** l'organisateur voit la grille, mais il ne peut ni fixer la date ni obtenir les groupes et leurs noms de salle. Sans groupes, ni le placement (3-3) ni la salle (3-4) ne peuvent exister (SPEC CAP-2).

**Approach:**
- **Migration `0003` :** elle crée `groups` et `assignments` au complet, puisqu'elle sera immuable une fois poussée.
- **Commandes `lock_event` et `unlock_event` :** elles suivent AD-4 *Verrouillage*. Un groupe non réapparié reste en sommeil, selon la décision de Vincent du 2026-11-07.
- **Noms de salle :** ils viennent du gabarit `ROOM_NAME_TEMPLATE`, par défaut `{label}-{code}`.
- **Interface :** une feuille Verrouiller dans Disponibilités, « Déverrouiller » dans Réglages, et les onglets Composition et Salle, minimaux tant que l'événement est `locked`.

## Boundaries & Constraints

**Always:**

**Migration `crates/server/migrations/0003_groups.sql`**

Elle suit le style de 0002 (commentaire de tête, `STRICT`, `foreign_keys`) :

- **`groups`.**
  - Colonnes :
    - `id` PK, `CHECK length = 22` ;
    - `event_id` → `events` `ON DELETE CASCADE` ;
    - `slot_id` → `slots` `ON DELETE CASCADE` ;
    - `occurrence INTEGER ≥ 1` ;
    - `retained INTEGER CHECK IN (0,1)` ;
    - `parties ≥ 1`, `party_size ≥ 1` ;
    - `room_label TEXT`, `room_name TEXT` ;
    - `created_at`, `CHECK length = 20`.
  - Contraintes : `UNIQUE(slot_id, occurrence)`, `UNIQUE(room_name)` et un index sur `event_id`.
- **`assignments`.**
  - Colonnes :
    - `candidature_id` → `candidatures` `ON DELETE CASCADE` ;
    - `group_id` → `groups` `ON DELETE CASCADE` ;
    - `party`, `party_index` et `bench_index`, tous nullables et ≥ 1 ;
    - `CHECK ((party IS NULL) = (party_index IS NULL) AND (party IS NULL) <> (bench_index IS NULL))`.
  - Les trois index uniques d'AD-4 : `UNIQUE(candidature_id)`, `UNIQUE(group_id, party, party_index)` et `UNIQUE(group_id, bench_index)`.
- Pas de table `confirmations` : elle arrive en epic 5.

**Configuration `ROOM_NAME_TEMPLATE`**
- Variable facultative, de défaut `{label}-{code}`.
- Elle est lue une fois et validée au démarrage, en fail-fast :
  - jetons permis : `{label}` et `{code}` seulement ;
  - `{code}` exactement une fois ;
  - littéraux en ASCII imprimable, sans `{` ni `}` ;
  - la partie fixe plus `{code}` (4 caractères) tient dans `room_label_max + 5` `[ASSUMPTION]`.
- Elle est ajoutée à `deploy/.env.example` (section des facultatives, avec un commentaire) et à `Config`. Les tests de configuration couvrent son défaut, sa surcharge et une valeur invalide nommée.

**Domaine**
- **`domain::room` :** `RoomTemplate::parse` et `compose(template, label, code)`. Le code est de 4 caractères Crockford (`0-9A-Z` sans `ILOU`) et vient d'une nouvelle méthode `IdGenerator::room_code()` : CSPRNG en production, séquence déterministe dans les tests.
- **`event_may` :** il gagne deux actions.
  - `Lock` : `cancelled` → `event.closed`, `locked` → `event.locked`.
  - `Unlock` : `cancelled` → `event.closed`, `open` → `event.not_locked`.
  - Le test exhaustif passe à une table littérale.

**`POST /api/organizer/lock`**

Corps `{slotIds: string[]}`, droit `OrganizerLink`, classe `write`, sans id dans l'URL.

- **Validation, dans l'ordre :**
  1. `event_may(Lock)` ;
  2. longueur de la liste entre 1 et 14, sinon `event.group_count_out_of_range` (400) ;
  3. chaque id appartient à l'événement, sinon `slot.not_found` ;
  4. chaque créneau est à venir, sinon `slot.in_past` avec `fields: slotIds.N` ;
  5. l'instance de l'événement existe dans la référence, même désactivée (AD-6), sinon `reference.unavailable`.
- **Écriture, dans une transaction :**
  - Chaque entrée k d'un créneau s est appariée au groupe existant `(s, k)` : il est réutilisé tel quel (disposition, nom, placements) et marqué `retained = 1`.
  - Une entrée sans groupe existant crée un groupe, avec `parties`, `party_size` et `room_label` figés depuis l'instance et un nom composé.
  - Collision de nom : nouveau code, cinq essais au plus, puis `server.internal` journalisé, sans le nom.
  - Tout autre groupe de l'événement passe à `retained = 0` : il est mis en sommeil, jamais supprimé.
  - Statut `locked`. Les statuts des candidatures ne changent pas.
  - Le serveur ne vérifie pas les disponibilités : un groupe incomplet est permis.
- **Réponse :** `OrganizerEvent`.

**`POST /api/organizer/unlock`**
- Corps `{}`.
- Refus : `event_may(Unlock)`, puis 409 `event.no_upcoming_slot` si aucun créneau n'a `starts_at > now`.
- Seul `status` change, vers `open`. Groupes, placements et noms sont conservés.

**Nouveaux codes**
- `event.group_count_out_of_range` (400), `event.not_locked` (409) et `event.no_upcoming_slot` (409), avec leurs textes fr et en.
- `event.not_locked` et `event.no_upcoming_slot` rejoignent les `STATE_CODES` d'`OrganizerPage`.

**DTO**
- `OrganizerEvent.groups: OrganizerGroup[]`, toujours présent, trié par début de créneau puis par `occurrence`.
  - Champs : `{id, slotId, occurrence, retained, parties, partySize, roomLabel, roomName}`.
  - Le libellé « G1 » est calculé côté client, dans cet ordre, sur les groupes `retained` uniquement.
- `OrganizerSlot` gagne `hasGroups`, vrai s'il existe un groupe sur ce créneau, même en sommeil.
- Le type est recopié à la main dans `types.ts`.
- `PublicEvent` et `ParticipantView` ne changent pas : le côté participant est la story 3-5.

**Feuille Verrouiller** (bouton principal « Verrouiller » dans Disponibilités, seulement en `open`)
- **Lignes :** une par créneau à venir. Chacune a une case « Retenir » et un Stepper de groupes.
  - Le Stepper commence à 1 et va de `1` à `max(1, floor(availableAccepted / (parties × partySize)))`, d'après l'instance de l'événement.
  - La ligne indique « Assez pour N groupes », N valant ce `floor`, 0 compris, au singulier ou au pluriel.
- **Pré-cochage :** les groupes `retained` du dernier verrouillage, s'il y en a. Sinon, le créneau qui a le plus d'`availableAccepted`, le plus tôt en cas d'égalité, avec 1 groupe.
- **Validation :**
  - le bouton « Verrouiller K groupes » envoie la liste avec les doublons répétés ;
  - il est désactivé quand K = 0, avec sa raison ;
  - deux touchers suffisent à partir du pré-cochage.

**Onglets**
- En `locked`, l'ordre est Candidatures, Disponibilités, Composition, Salle, Réglages.
- En `open`, Composition et Salle disparaissent. Si l'onglet courant disparaît, l'affichage revient sur Disponibilités.
- En 3-2, ces deux onglets montrent la liste des groupes `retained`, du type « G1 · jeu. 21:00 · LUDRA-K7Q2 », sans action. Le contenu réel arrive en 3-3 et 3-4.
- La grille est en lecture seule en `locked`, sans bouton Verrouiller.

**« Déverrouiller »** (Réglages, seulement en `locked` et s'il reste un créneau à venir)
- Il demande une `ConfirmSheet` : « Les joueurs ne verront plus leur placement. Groupes et noms de salle sont gardés. »

**Suppression d'un créneau qui a des groupes** (en `open`, depuis Réglages)
- La confirmation nomme la perte : « N groupes de ce créneau seront supprimés, avec leurs placements. »

**Commentaires**
- Les commentaires « Aucun groupe / aucune ligne `assignments` avant l'epic 3 » de `remove_slot`, `set_candidature_status` et `withdraw_candidature` sont mis à jour.
- Le `DELETE` de `assignments` au retrait et au refus est ajouté dès maintenant, dans la même transaction (AD-4), avec un test.

**Locales**
- Tous les textes en fr et en en, ton *Voice and Tone*.

**Never:**
- Toucher aux migrations 0001, 0002 et 9999.
- Créer `confirmations`.
- Supprimer un groupe au re-verrouillage.
- Écrire des placements (3-3).
- Livrer le contenu de Salle ou « Régénérer le nom » (3-4).
- Modifier la vue participant (3-5).
- Mettre un id dans l'URL.
- Pousser cette story sans la relecture de la migration par Vincent.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Verrouiller | `open`, `[A, A, B]` à venir | 3 groupes (A#1, A#2, B#1), disposition figée, 3 noms distincts `LUDRA-XXXX`, statut `locked` | — |
| Collision | le générateur renvoie un code déjà pris, puis un code libre | le second code est retenu | 5 collisions → 500 `server.internal` |
| Liste invalide | `[]` ou 15 entrées ; créneau étranger ; créneau passé | rien n'est écrit | 400 `event.group_count_out_of_range` ; 404 `slot.not_found` ; 400 `slot.in_past` + `fields` |
| Déjà verrouillé ou annulé | `locked` ; `cancelled` | rien n'est écrit | 409 `event.locked` ; 409 `event.closed` |
| Déverrouiller | `locked`, un créneau à venir | `open` ; groupes, noms et placements intacts | aucun créneau à venir → 409 `event.no_upcoming_slot` ; `open` → 409 `event.not_locked` |
| Re-verrouiller moins | groupes A#1 et A#2 existants, puis `[A]` | A#1 réutilisé (même nom), A#2 `retained = 0`, conservé | — |
| Re-verrouiller plus | A#1 existant, puis `[A, A]` | A#1 garde son nom ; seul A#2 est nommé | — |
| Suppression de créneau | `open`, créneau avec groupes | groupes et placements supprimés en cascade | — |
| Gabarit invalide | `ROOM_NAME_TEMPLATE` sans `{code}` ou avec `{cp}` | démarrage refusé, variable nommée | `ConfigError::Invalid` |

</intent-contract>

## Code Map

- `crates/server/migrations/0002_events.sql` -- style à suivre. Lecture seule.
- `crates/server/tests/events_create.rs:498-531` et `tests/sqlite.rs:60-84` -- l'un vérifie l'absence de `groups` et d'`assignments`, l'autre liste exactement les tables. Les deux sont à ajuster : le premier doit porter sur les versions ≤ 2, le second doit inclure les deux tables.
- `deploy/plan-deploy.sh:54-62` -- une migration ajoutée déclenche déjà la sauvegarde.
- `crates/domain/src/event.rs:591-620,1331-1360` -- `EventAction`, `EventRefusal`, `event_may` et son test exhaustif.
- `crates/domain/src/counts.rs:88-140` -- `slot_coverage` (`available_accepted`).
- `crates/server/src/infra/events.rs:83,123,198,293,308` -- `EventSnapshot`, `load`, `status_and_candidature_count`, `delete_slot` et `cancel`, modèles pour les écritures.
- `crates/server/src/infra/ids.rs` -- `IdGenerator` et `DeterministicIdGenerator`, plus `room_code()`.
- `crates/server/src/app/commands/{cancel_event,remove_slot,set_candidature_status,withdraw_candidature}.rs` et `commands/mod.rs:18-45` -- modèle de commande, commentaires à mettre à jour et `EventWriteError`.
- `crates/server/src/http/{organizer,mod,errors,dto}.rs` -- routes du groupe `write` (`mod.rs:78-104`), registre des codes, `OrganizerEvent` et `OrganizerSlot`.
- `crates/server/src/config.rs:93-205,268-365` -- motif `parse_or`, `ConfigError::Invalid` et les tests de configuration.
- `reference/instances.toml` -- `room_label_charset`, `room_label_max = 8`, `ludra` en 2×5 avec `LUDRA`.
- `web/src/pages/OrganizerPage.tsx:50-68,91,239-304,379-445` -- `STATE_CODES`, `TABS`, `gridChoice`, `command`, `settingsActions`, tablist et panneaux.
- `web/src/components/{AvailabilityGrid,EventSettings,Stepper,ConfirmSheet,Sheet}.tsx` -- grille, Réglages (créneaux `:530-608`, annulation `:612-655`), Stepper et feuilles.
- `web/src/api/{organizer,types}.ts`, `web/src/locales/*.json` et `web/src/test/fixtures.ts`.
- `crates/server/tests/organizer.rs:150-158` et `tests/organizer_settings.rs` -- test des clés de premier niveau et motifs de test.

## Tasks & Acceptance

**Execution:**
- `crates/server/migrations/0003_groups.sql` -- les deux tables et leurs index.
- `crates/domain/src/room.rs` (nouveau) et `crates/domain/src/event.rs` -- gabarit et composition, `Lock`/`Unlock`, et leurs tests.
- `crates/server/src/config.rs` et `deploy/.env.example` -- `ROOM_NAME_TEMPLATE`.
- `crates/server/src/infra/{ids,groups,events}.rs` -- `room_code`, lecture et écriture des groupes, statut.
- `crates/server/src/app/commands/{lock_event,unlock_event}.rs` -- les deux commandes. Ajouter aussi le `DELETE` d'`assignments` dans `withdraw_candidature` et `set_candidature_status` pour `rejected`.
- `crates/server/src/app/queries/organizer_view.rs`, `http/{organizer,mod,dto,errors}.rs` -- `groups`, `hasGroups`, les routes et les 3 codes.
- `crates/server/tests/organizer_lock.rs` -- chaque ligne de la matrice, avec un générateur de codes scripté pour la collision, plus le quota `write` et des journaux sans secret.
- `web/src/components/LockSheet.tsx`, `AvailabilityGrid.tsx`, `EventSettings.tsx`, `pages/OrganizerPage.tsx` et `api/*` -- feuille, onglets, Déverrouiller et confirmation de suppression.
- `web/src/locales/fr.json` et `en.json`.
- `web/src/**/*.test.tsx` -- tests :
  - pré-cochage et borne du Stepper ;
  - envoi des doublons ;
  - « Verrouiller K groupes » ;
  - onglets selon le statut, avec le retour sur Disponibilités ;
  - Composition et Salle minimaux ;
  - Déverrouiller confirmé et masqué sans créneau à venir ;
  - confirmation de suppression d'un créneau avec groupes ;
  - grille sans bouton en `locked`.

**Acceptance Criteria:**
- Étant donné un événement `open` avec 12 acceptés sur jeudi 21:00, quand l'organisateur ouvre la feuille Verrouiller, alors le Stepper de jeudi est borné à 1 (12 / 10) et « Verrouiller 1 groupe » se fait en deux touchers.
- Étant donné un verrouillage, un déverrouillage, puis un re-verrouillage identique, quand on relit `groups`, alors les noms de salle sont inchangés.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 32 findings — high 0, medium 9, low 21, false 2, maybe-false 0
- findings:
  - `[low]` `[patch]` Total de groupes non plafonné à 14 côté client — bouton désactivé au-delà, avec sa raison ; testé.
  - `[low]` `[patch]` La limite de 14 recopiée à trois endroits — `GROUPS_MAX = SLOTS_MAX`, et message à paramètre `{max}`.
  - `[low]` `[reject]` Instance recherchée même si aucun groupe n'est nouveau — il faut une référence absente et un re-verrouillage identique ; cas rare.
  - `[medium]` `[patch]` Gabarit vérifié seulement au démarrage, après la migration, et sans test ; rechargement non vérifié — fonction pure `check_room_template` testée, appelée avant la migration et au rechargement (jeu refusé, précédent gardé).
  - `[medium]` `[defer]` Le placement d'un groupe en sommeil occupe `UNIQUE(candidature_id)` — à trancher dans `apply_placement` (3-3), qui remplace le placement complet. Le schéma n'empêche aucune issue.
  - `[medium]` `[patch]` Pas de cohérence inter-événements dans 0003 — clés étrangères composées `groups (slot_id, event_id)` et `assignments` (`event_id`, vers `candidatures` et `groups`) ; test de refus en base.
  - `[medium]` `[patch]` Re-verrouiller après un déverrouillage met en sommeil les groupes des créneaux passés — ces groupes ne sont plus touchés ; testé en domaine et en HTTP.
  - `[low]` `[patch]` Groupes en sommeil invisibles mais comptés — ligne « N groupe(s) en sommeil » dans Composition et Salle, et confirmation de suppression précisée.
  - `[low]` `[patch]` Créneau devenu passé pendant que la feuille est ouverte — les lignes nommées dans `fields` sont retirées et l'erreur est affichée ; testé.
  - `[low]` `[patch]` Stepper actif pendant l'envoi — prop `disabled`.
  - `[low]` `[patch]` État modifié pendant le rendu dans `OrganizerPage` — onglet effectif dérivé, effets à la place.
  - `[low]` `[reject]` Libellé vide ou non ASCII dans `compose` — la référence valide le libellé contre `room_label_charset`.
  - `[low]` `[patch]` Commentaire de `.env.example` ambigu — réécrit.
  - `[false]` `[reject]` Fichier de story absent du diff — volontaire : le diff de relecture exclut `_bmad-output`.
  - `[medium]` `[patch]` Rechargement de la référence non vérifié (Edge Case Hunter) — même correctif que le gabarit.
  - `[low]` `[reject]` Littéraux et code hors `room_label_charset` — ce charset borne le libellé ; le `-` littéral vient du gabarit choisi par Vincent.
  - `[low]` `[patch]` Plafond de 14 (Edge Case Hunter) — même correctif que le plafond côté client.
  - `[low]` `[patch]` Créneau passé pendant que la feuille est ouverte (Edge Case Hunter) — même correctif.
  - `[medium]` `[patch]` Vérification de longueur au démarrage non testée — même correctif que le gabarit.
  - `[low]` `[patch]` Ordre des groupes jamais testé quand l'ordre de création diffère — test `lock([a, b, a])`.
  - `[low]` `[patch]` Fermeture automatique de la feuille au rafraîchissement non testée — test ajouté.
  - `[low]` `[patch]` Bouton Verrouiller non vérifié absent sur un raid annulé — test ajouté.
  - `[medium]` `[patch]` Rechargement de la référence non vérifié (Verification Gap) — même correctif que le gabarit.
  - `[low]` `[patch]` Vérification faite après `db.migrate()` — elle passe avant.
  - `[low]` `[reject]` « Placements gardés » prouvé par du SQL brut — `apply_placement` arrive en 3-3.
  - `[low]` `[reject]` `reference.unavailable` testé avant `event_may` — ce cas n'existe que sans référence chargée ; l'effet est sans conséquence.
  - `[medium]` `[patch]` Vérification de longueur sautée sans référence au démarrage — même correctif que le gabarit (vérification au rechargement).
  - `[low]` `[patch]` Groupes en sommeil comptés sans être visibles (audit d'intention) — même correctif que la ligne en sommeil.
  - `[low]` `[reject]` Un groupe en sommeil est repris en silence — c'est le réappariement automatique d'AD-4 `(slot, occurrence)` ; la ligne « en sommeil » le signale.
  - `[false]` `[reject]` La promesse « masqué aux participants » n'est pas testée — hors périmètre : la vue participant est la story 3-5.
  - `[medium]` `[defer]` Lignes `assignments` des groupes en sommeil face à `apply_placement` (audit d'intention) — même report vers 3-3.
  - `[low]` `[reject]` `{cp}` retiré de la grammaire et marge fixée à +5 — décision de Vincent (sans CP) ; la marge reste un `[ASSUMPTION]` réglable.

## Design Notes

**Groupes en sommeil :** AD-4 dit à la fois « K = longueur de la liste » et « déverrouiller ne supprime rien ». La colonne `retained` réconcilie les deux, selon la décision de Vincent : seuls les groupes retenus forment le verrouillage courant, et les autres attendent avec leurs placements.

**`UNIQUE(room_name)` global :** « unique parmi les groupes non purgés » revient à un index global, puisque la purge (epic 5) supprime les lignes.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert, JS sous 150 KB.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh && sh deploy/tests/plan-deploy.test.sh` -- expected: OK.

## Auto Run Result

Status: done

**Résumé :**
- **Migration `0003` :** elle crée `groups` (avec `retained`) et `assignments`, avec les trois index uniques d'AD-4 et des clés étrangères composées qui empêchent de mélanger deux événements. Elle ne crée pas `confirmations`.
- **Verrouillage (`lock_event`, `POST /api/organizer/lock`) :** les groupes sont réappariés par `(créneau, occurrence)`. Un groupe non repris est mis en sommeil, jamais supprimé, et les groupes des créneaux passés ne sont pas touchés. Chaque groupe a un nom de salle `{label}-{code}` unique, avec un nouvel essai en cas de collision.
- **Déverrouillage (`unlock_event`) :** il est refusé quand aucun créneau n'est à venir.
- **Gabarit de nom de salle :** `ROOM_NAME_TEMPLATE` est validé au démarrage, avant la migration, et à chaque rechargement de la référence.
- **Feuille Verrouiller :** les Stepper sont bornés aux disponibles divisés par la taille d'un groupe, avec un pré-cochage et un plafond total de 14.
- **Interface :** les onglets Composition et Salle, minimaux, n'apparaissent qu'en `locked`. Réglages gagne « Déverrouiller ». La confirmation de suppression d'un créneau nomme la perte des groupes.
- **Placements :** retrait et refus suppriment le placement.

**Relecture :** 32 remarques, dont 22 corrigées (13 correctifs), 2 reportées vers la 3-3 (le placement dans un groupe en sommeil) et 8 écartées (dont 2 fausses), chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** oui. Plusieurs entrées de gravité moyenne ont été corrigées, dont la migration. Le risque non vérifié est le comportement des clés étrangères composées face aux insertions de la 3-3.

**Vérification :**
- fmt et clippy `-D warnings` passent ; `REQUIRE_WEB_DIST=1 cargo test` : 319 tests, 0 échec.
- `list-error-codes` (44 codes), `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (359 tests). Le budget est respecté : 113,6 sur 150 ko de JS.

**Risques résiduels :**
- **La migration `0003` n'est pas poussée.** Vincent doit la relire, car elle devient immuable une fois sur `main`.
- La marge de longueur du nom de salle (`room_label_max + 5`) est un `[ASSUMPTION]`.
