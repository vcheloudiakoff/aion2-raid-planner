---
title: 'Placement à la main'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '5644efa80f4e675634bb9043ff0ef3200afabc3b'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/placement-et-salle.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/stories/3-2-verrouiller-et-deverrouiller.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred:
  - summary: >-
      SPEC CAP-3, placement-et-salle.md et la spine (AD-4) disent « le placement complet de l'événement, remplacé en entier » ; apply_placement ne remplace que les groupes retenus.
    evidence: |-
      Conséquence de la décision de Vincent (groupes en sommeil gardant leurs placements). Le texte des trois documents doit être aligné par leurs writers (bmad-spec, bmad-architecture), sans changement de code.
    location: >-
      _bmad-output/specs/spec-epic-3-organiser/SPEC.md
    severity: medium
---

<intent-contract>

## Intent

**Problem:** l'organisateur obtient des groupes verrouillés, mais il ne peut pas y placer ses titulaires ni son banc (SPEC CAP-3).

**Approach:**
- **Serveur :** une commande `apply_placement` qui remplace le placement des groupes retenus. Elle ne refuse que les violations structurelles. Elle renvoie dans `OrganizerEvent` les placements et les avertissements, calculés dans `domain` à la lecture.
- **Web :** un onglet Composition chargé à la demande (chunk séparé), qui travaille sur un brouillon local. Toucher une cible ouvre la réserve. Un slot occupé propose Retirer, Au banc et Échanger. Le bouton Enregistrer envoie le placement complet. Un bandeau signale les données changées sans écraser le brouillon.

## Boundaries & Constraints

**Always:**
- **`POST /api/organizer/placement`**
  - Accès : `OrganizerLink`, classe `write`, corps lu par `FormJson`, aucun id dans l'URL.
  - Corps : `{placements: [{candidatureId, groupId, party?, index?, benchIndex?}]}`. Les nombres sont des `i64`, donc une valeur hors bornes donne une erreur de champ et non `form.malformed`.
- **Statut de l'événement.**
  - `locked` seulement, par une nouvelle action `EventAction::Place` : `open` donne `event.not_locked` (409), `cancelled` donne `event.closed`.
  - Pas de restriction sur un créneau déjà commencé.
- **Violations structurelles.** Elles sont toutes en 400, avec `fields` sur `placements.N.<champ>`, et sont vérifiées dans une fonction pure `domain::placement::validate` :
  - candidature inconnue, d'un autre événement ou non `accepted` → `placement.candidature_not_placeable` ;
  - même candidature deux fois → `placement.duplicate_candidature` ;
  - groupe inconnu, d'un autre événement ou en sommeil (`retained = 0`) → `placement.unknown_group` ;
  - ni (`party` + `index`) ni `benchIndex`, les deux à la fois, `party ∉ [1, parties]` ou `index ∉ [1, party_size]` du groupe figé → `placement.out_of_layout` ;
  - deux entrées sur le même `(groupe, party, index)` ou le même `(groupe, benchIndex)` → `placement.slot_taken`.
  - **Jamais d'erreur** pour une indisponibilité ou une borne de classe : ce sont des avertissements.
- **Remplacement.** Il se fait dans une transaction.
  - Suppression des `assignments` des groupes retenus, puis de toute ligne d'une candidature présente dans le corps, y compris dans un groupe en sommeil.
  - Insertion des nouvelles lignes, avec le `benchIndex` renuméroté de façon contiguë dans chaque groupe, dans l'ordre reçu.
  - Les placements des groupes en sommeil dont les candidatures sont absentes du corps sont conservés. Ils réapparaîtront si le groupe est repris (décision de Vincent sur les groupes en sommeil).
  - Réponse : 200 avec `OrganizerEvent`. La dernière écriture gagne (AD-16).
- **DTO.** Chaque `OrganizerGroup` gagne trois champs, calculés par `domain::placement::warnings` :
  - `starters: [{candidatureId, party, index, unavailable}]` ;
  - `bench: [{candidatureId, benchIndex, unavailable}]` ;
  - `classWarnings: {classId: {count, min, max}}`, seulement pour les classes hors bornes, comptées sur les titulaires `[ASSUMPTION]`.

  `unavailable` est vrai quand la candidature n'a pas coché le créneau du groupe. Le type est recopié à la main dans `types.ts`. `PublicEvent` et `ParticipantView` ne changent pas.
- **Nouveaux codes.** Les cinq `placement.*`, en 400, avec leurs textes fr et en. Un 400 structurel garde le brouillon.
- **Chunk.** L'onglet Composition est un module `React.lazy` (`components/Composition.tsx` et ses sous-modules : réserve, actions de slot), avec un squelette en fallback.
  - Le budget initial reste ≤ 150 KB. Rien du module paresseux n'est importé statiquement par l'entrée.
  - Le brouillon et le calcul du bandeau vivent dans `OrganizerPage`, pour survivre à un changement d'onglet. Ils sont effacés au changement de secret.
- **Écran Composition** (en `locked`, 360 px d'abord).
  - **Puces de groupe :** « G1 · jeu. 21:00 », sur les groupes retenus, à sélection unique.
  - **Ligne d'état :** sous les puces, par exemple « 8 / 10 · manque 1 Soin » (manque calculé sur les minimums par rôle, comme les comptes publics).
  - **Slots :** Équipe 1 puis Équipe 2, slots 1 à `party_size`, en `room-slot` (52 px de haut au minimum).
    - Contenu : le numéro, puis le rôle du titulaire en `meta` et `Pseudo[SRV]` en `mono`, ou « libre » en `ink-disabled`.
    - Avertissements en ligne : « pas dispo à ce créneau », et « 3 / max 2 » sur la ligne de classe.
  - **Banc :** ordonné, avec « + Ajouter au banc ».
  - **Barre d'action** fixée en bas :
    - en brouillon : « Enregistrer » (principal) et « Abandonner », qui demande une confirmation ;
    - hors brouillon : vide, avec un emplacement prêt pour « Composer automatiquement » (epic 4), sans ce bouton.
- **Gestes.**
  - **Cible libre** (slot ou banc) : un toucher ouvre la réserve dans une `Sheet`.
  - **Réserve :** candidatures `accepted` seulement, en trois sections.
    1. Disponibles à ce créneau et non placées dans un groupe retenu, la classe qui manque d'abord, puis le CP décroissant, puis l'ordre de création.
    2. « Placées dans un autre groupe » : un toucher déplace la candidature.
    3. « Pas dispo à ce créneau », repliée par défaut.
  - **Slot occupé :** « Retirer », « Au banc » ou « Échanger ». Échanger demande de toucher ensuite un autre slot, et les deux occupants sont permutés.
  - **Unicité :** une candidature n'est jamais dans deux slots. L'interface déplace au lieu de dupliquer.
  - **Envoi :** toute modification crée le brouillon. Rien n'est envoyé avant « Enregistrer ». « Enregistrer » envoie le placement complet des groupes retenus.
- **Bandeau « N candidatures ont changé depuis ton brouillon ».**
  - Le calcul compare la vue de base du brouillon à la vue rafraîchie : statut, classe, CP ou disponibilités changés, candidatures ajoutées ou retirées, placement serveur modifié.
  - Il affiche « Voir », qui recharge et abandonne le brouillon après confirmation. « Enregistrer » reste possible.
  - Le brouillon n'est jamais écrasé.
  - À l'enregistrement, une entrée devenue non `accepted` donne le 400 structurel, affiché sans perdre le brouillon.
- **Retrait ou refus.** Le placement est supprimé, ce que la 3-2 fait déjà. La ligne disparaît de l'écran au rafraîchissement suivant.
- **Locales.** Tous les textes sont en fr et en en.

**Never:**
- Ajouter une migration.
- Livrer le solveur ou le bouton « Composer automatiquement ».
- Livrer le contenu de l'onglet Salle ou le message Discord (3-4).
- Modifier la vue participant (3-5).
- Utiliser le glisser-déposer ou le balayage.
- Faire du défilement horizontal.
- Refuser un placement pour une indisponibilité ou une borne.
- Écraser le brouillon au rafraîchissement.
- Mettre un id dans l'URL.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Placement nominal | `locked`, 5 titulaires en Équipe 1 et 1 au banc dans G1 | 200 ; lignes écrites, `benchIndex` = 1 | — |
| Hors dispo / hors borne | titulaire qui n'a pas coché le créneau ; 3 Clercs pour un max de 2 | 200 ; `unavailable = true` ; `classWarnings.cleric = {count:3, max:2}` | — |
| Structure | doublon de candidature ; `party = 3` ; deux entrées sur le même slot ; candidature `pending` | rien n'est écrit | 400 `placement.*`, avec `fields` |
| Groupe en sommeil | `groupId` d'un groupe `retained = 0` | rien n'est écrit | 400 `placement.unknown_group` |
| Remplacement | ancien placement A dans G1 ; corps sans A | A est retiré de G1 | — |
| Sommeil conservé | B placée dans G2 en sommeil, absente du corps | la ligne de B dans G2 est gardée | — |
| Sommeil déplacé | B placée dans G2 en sommeil, présente dans le corps sur G1 | B passe dans G1, sa ligne G2 est supprimée | — |
| Statut | `open` ; `cancelled` | rien n'est écrit | 409 `event.not_locked` ; 409 `event.closed` |

</intent-contract>

## Code Map

- `crates/server/migrations/0003_groups.sql:45-62` -- `assignments` (event_id, clés étrangères composées, index uniques). En lecture seule.
- `crates/server/src/infra/groups.rs:9-126` -- `GroupRow`, `list`, `set_retained`, `delete_assignment`. À ajouter : lecture, remplacement et insertion des `assignments`.
- `crates/server/src/app/commands/lock_event.rs:62-139` et `commands/mod.rs:21-53` -- le modèle de commande et `EventWriteError`.
- `crates/server/src/app/queries/organizer_view.rs:49-154` -- `load` ; ajouter les placements et les avertissements par groupe.
- `crates/server/src/http/dto.rs:444-514,569` -- `OrganizerEvent`, `OrganizerGroup`, `OrganizerCandidature` et le modèle de corps `LockEvent`.
- `crates/domain/src/counts.rs:43-140` -- `role_of`, la logique `missing_roles` et `slot_coverage`.
- `crates/domain/src/event.rs:592-631,1460-1494` -- `EventAction`, `event_may` et son test exhaustif (à étendre avec `Place`).
- `crates/server/src/http/errors.rs:45-155` -- registre des codes ; `placement` figure déjà dans `ENTITIES`.
- `crates/server/src/http/{organizer,mod}.rs` -- motif `post_lock` et `respond`, routeur `write`.
- `crates/server/tests/organizer_lock.rs:110-202` et `tests/common/candidatures.rs` -- `setup`, `lock`, `group_id`, `place`, `assignments`, `candidate`, `set_status`. `tests/organizer.rs:150-158` vérifie les clés de premier niveau.
- `web/src/pages/OrganizerPage.tsx:57-73,158-290,462-463` -- `STATE_CODES`, `LOCKED_TABS`, `apply`, rafraîchissement, `command` et le placeholder Composition.
- `web/src/components/{GroupList,Sheet,ConfirmSheet}.tsx` -- `retainedGroups`, numérotation G et feuilles.
- `web/scripts/check-budget.mjs` et `check-budget.test.mjs:41` -- les chunks paresseux sont exclus du budget initial.
- `web/src/test/fixtures.ts:150` -- `organizerEvent` (`groups: []`), à compléter.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/placement.rs` (nouveau) et `domain/src/event.rs` -- `validate`, `warnings` et `EventAction::Place`, avec leurs tests unitaires.
- `crates/server/src/infra/groups.rs` et `app/commands/apply_placement.rs` -- lecture, remplacement et commande.
- `crates/server/src/app/queries/organizer_view.rs`, `http/{dto,organizer,mod,errors}.rs` -- DTO, route et les cinq codes.
- `crates/server/tests/organizer_placement.rs` -- chaque ligne de la matrice, plus le quota `write` et les journaux sans secret.
- `web/src/components/Composition.tsx` (paresseux), et `ReserveSheet`, `RoomSlot` et `SlotActions` (ou équivalents) -- l'écran.
- `web/src/pages/OrganizerPage.tsx`, `web/src/api/{organizer,types}.ts`, `web/src/locales/*.json` et `web/src/test/fixtures.ts`.
- Tests web dans `web/src/**/*.test.tsx`, chacun portant sur un comportement :
  - réserve : ordre et sections ;
  - déplacement d'une candidature déjà placée ;
  - Retirer, Au banc et Échanger ;
  - brouillon créé à la première modification, rien n'est envoyé avant Enregistrer ;
  - Abandonner avec confirmation ;
  - bandeau sans écrasement ;
  - avertissements en ligne ;
  - 400 structurel qui garde le brouillon ;
  - brouillon conservé au changement d'onglet ;
  - chunk chargé à la demande (rendu à travers `Suspense`).

**Acceptance Criteria:**
- Étant donné un raid verrouillé à 1 groupe, quand l'organisateur place 10 titulaires et 1 joueur au banc puis enregistre, alors `GET /api/organizer` renvoie ces 11 placements. Les avertissements de ceux qui n'avaient pas coché le créneau sont renvoyés, sans aucune erreur.
- Étant donné le build web, quand `check-budget` s'exécute, alors le JS initial reste sous 150 KB et Composition est dans un chunk séparé.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 29 findings — high 0, medium 8, low 21, false 0, maybe-false 0
- findings:
  - `[medium]` `[patch]` Le remplacement n'est jamais testé entre deux événements — test ajouté : placer dans A ne touche pas aux lignes de B.
  - `[low]` `[patch]` Avertissements du banc non testés — tests ajoutés : banc indisponible, serveur et brouillon ; banc hors du décompte des classes.
  - `[low]` `[patch]` Un changement de classe seul n'est pas testé dans `changedCount` — test ajouté.
  - `[medium]` `[patch]` Le brouillon survit à un déverrouillage suivi d'un re-verrouillage — brouillon abandonné, avec avis, si le statut quitte `locked` ou si les groupes retenus changent ; testé.
  - `[medium]` `[patch]` Brouillon périmé après un déverrouillage (Blind Hunter) — même correctif.
  - `[medium]` `[patch]` Les `fields` d'un 400 structurel ne sont pas rattachés aux places — places fautives signalées en ligne ; testé.
  - `[medium]` `[patch]` Un joueur qui n'est plus accepté n'est pas signalé dans le brouillon — « plus acceptée » ou « absente » ; testé.
  - `[low]` `[patch]` `missingRoles` faux sans référence — liste vide ; testé.
  - `[low]` `[patch]` La réserve cache un placement en groupe en sommeil — note « dans un groupe en sommeil ».
  - `[low]` `[patch]` Accessibilité des slots — `aria-label` complet, `aria-pressed` sur la source, plus d'`aria-haspopup` pendant un échange.
  - `[low]` `[patch]` Le mode échange survit à l'enregistrement, contrôles actifs pendant l'envoi, pas d'Échap — échange effacé à l'enregistrement, à l'abandon et à Échap ; contrôles désactivés pendant l'envoi.
  - `[low]` `[reject]` `DELETE` en boucle — borné par les candidatures acceptées, au plus 200, dans une transaction ; coût négligeable.
  - `[low]` `[patch]` Nom de test qui promet plus qu'il ne vérifie — borne `min: 1` ajoutée.
  - `[low]` `[patch]` `benchIndex` sans borne supérieure — refusé au-delà de 200 (`out_of_layout`).
  - `[low]` `[patch]` Vocabulaire anglais incohérent — « sign-ups » et « empty ».
  - `[low]` `[patch]` Commentaires faux ou trop longs — ordre de tri corrigé, lignes repliées.
  - `[low]` `[patch]` Chemins du brouillon non testés — échec sur code d'état et échec réseau testés.
  - `[medium]` `[patch]` Brouillon périmé (Edge Case Hunter) — même correctif.
  - `[medium]` `[patch]` Joueur plus accepté non signalé (Edge Case Hunter) — même correctif.
  - `[low]` `[patch]` `missingRoles` sans référence (Edge Case Hunter) — même correctif.
  - `[low]` `[reject]` Entier JSON au-delà d'`i64::MAX` donne `form.malformed` — aucun client ne l'envoie ; un JSON hors type reste une requête mal formée.
  - `[low]` `[patch]` Échange actif après l'enregistrement (Edge Case Hunter) — même correctif que le mode échange.
  - `[low]` `[patch]` Une place remplie par un rafraîchissement est écrasée — `placeAt` refuse une place prise ; la réserve se ferme et les actions s'ouvrent ; testé.
  - `[medium]` `[defer]` SPEC, compagnon et spine disent « placement complet de l'événement » ; la story applique le remplacement aux seuls groupes retenus (groupes en sommeil, décision de Vincent) — texte à aligner par `bmad-spec` et `bmad-architecture`.
  - `[low]` `[reject]` Pas de barre d'action hors brouillon — l'emplacement est marqué dans le code ; l'epic 4 rendra la barre avec son bouton.
  - `[low]` `[reject]` Deux copies des règles d'avertissement — le brouillon n'existe que côté client, qui doit donc recalculer ; les règles sont les mêmes et testées de chaque côté.
  - `[low]` `[reject]` Pas de test de contrat entre client et serveur — c'est la règle d'AD-5 (miroir à la main).
  - `[low]` `[reject]` L'existence du chunk Composition n'est pas vérifiée par un script — vérifiée au build (`Composition-*.js`, 4,3 ko) ; le budget initial est contrôlé.
  - `[low]` `[reject]` Mise en page non testée dans un vrai navigateur — vérification manuelle sur téléphone, comme les stories précédentes.

## Design Notes

**« Remplace en entier » porte sur le verrouillage courant :** le placement complet que voit l'organisateur est celui des groupes retenus. Les groupes en sommeil sont masqués et gardent leurs placements, comme l'a décidé Vincent. Une candidature déplacée d'un groupe en sommeil vers un groupe retenu y quitte sa ligne, ce qu'impose `UNIQUE(candidature_id)`.

**Rôle d'un slot :** la référence ne définit pas de rôle attendu par slot. Le slot affiche donc le rôle de son titulaire (EXPERIENCE), et « libre » sinon.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert ; JS initial sous 150 KB, Composition dans un chunk séparé.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.

## Auto Run Result

Status: done

**Résumé :**
- **`apply_placement`** (`POST /api/organizer/placement`) n'est disponible qu'en `locked`. Il ne refuse que les violations structurelles, via 5 codes `placement.*`.
- **Remplacement :** il porte sur les groupes retenus. Les placements des groupes en sommeil sont conservés, sauf quand on déplace leur joueur.
- **Avertissements :** « pas dispo » et « n / min / max » de classe sont calculés dans `domain` et renvoyés par `OrganizerEvent.groups`.
- **Onglet Composition :** il est chargé à la demande, dans un chunk séparé. Il contient :
  - un brouillon local qui survit aux changements d'onglet et est abandonné si les groupes changent ;
  - une réserve en trois sections ;
  - les actions Retirer, Au banc et Échanger ;
  - un bandeau « N candidatures ont changé » qui n'écrase pas le brouillon ;
  - les places fautives d'un 400 signalées en ligne.

**Relecture :** 29 remarques, dont 22 corrigées en 15 correctifs, 1 reportée (alignement du texte du SPEC et de la spine) et 6 écartées, chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** oui. Plusieurs entrées de gravité moyenne ont été corrigées. Le risque non vérifié est le cycle de vie du brouillon face aux rafraîchissements pendant un échange.

**Vérification :**
- fmt et clippy `-D warnings` passent ; `REQUIRE_WEB_DIST=1 cargo test` : 341 tests, 0 échec.
- Les contrôles annexes passent : `list-error-codes` (49 codes), crates réseau, durcissement (86 tests) et `plan-deploy` (20 tests).
- Web : tsc et vitest passent (390 tests). Le JS initial pèse 117,0 ko sur 150, avec Composition dans un chunk à part.

**Risques résiduels :**
- Rien n'est vérifié sur un vrai téléphone.
- Commit local non poussé, puisque la migration 0003 attend la relecture de Vincent.
