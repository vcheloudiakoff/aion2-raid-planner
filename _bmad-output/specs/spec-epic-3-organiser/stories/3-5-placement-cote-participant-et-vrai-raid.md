---
title: 'Placement côté participant et vrai raid'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '2ee0f0742b79c48a0c899a0445d0e704ae42f5b7'
review_loop_iteration: 0
followup_review_recommended: false
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/placement-et-salle.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/stories/3-3-placement-a-la-main.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-3-ma-candidature.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/deploy/README.md'
warnings: []
deferred: []
---

<intent-contract>

## Intent

**Problem:** un joueur accepté ne sait pas, en rouvrant `/p`, s'il joue, où, quand ni avec qui. Le signal de succès de l'epic 3 (un vrai raid monté de bout en bout) n'a pas encore de procédure de vérification (SPEC CAP-6, signal de succès).

**Approach:**
- **Serveur :** `ParticipantView` gagne le placement de la candidature dans un groupe retenu. Il porte le créneau, le nom de salle et les coéquipiers, selon AD-4 amendé le 2026-11-07.
- **Page `/p` :** elle affiche le placement, les coéquipiers et l'état « revenu en préparation ».
- **`deploy/README.md` §16 :** la liste de vérification du vrai raid, à remettre à Vincent.

## Boundaries & Constraints

**Always:**
- **`ParticipantView.placement`** : présent seulement si l'événement est `locked`, que la candidature est `accepted` et qu'elle a une ligne `assignments` dans un groupe `retained = 1`. Sinon la clé est absente (AD-5).
  - Forme : `{groupNumber, slotId, startsAt, roomName, seat}`.
    - `seat` vaut `{kind: "starter", party, index}` ou `{kind: "bench", benchIndex}`.
    - `groupNumber` est le rang du groupe parmi les groupes retenus, dans l'ordre créneau puis occurrence, comme « G1 » côté organisateur.
  - Un placement dans un groupe en sommeil vaut « non placé » : la clé est absente.
- **`ParticipantView.teammates`** : présent dans les mêmes conditions que `placement`, sinon absent. Chaque élément est `{pseudoSrv, class, party}` et rien d'autre.
  - **Titulaire :** les autres titulaires de la même équipe (même groupe, même `party`), triés par slot.
  - **Joueur au banc :** les titulaires des deux équipes de son groupe, triés par équipe puis par slot.
  - Ne figurent jamais dans `teammates` : le joueur lui-même, les joueurs au banc (pour personne), un autre groupe, une candidature non `accepted`.
- **`ParticipantView.backInPreparation`** : `true` si l'événement est `open` et qu'il a au moins un groupe. Cela veut dire qu'il a été verrouillé puis déverrouillé. Dans tous les autres cas, la clé est absente.
- **Recopie** à la main de ces champs dans `types.ts`. `PublicEvent` ne change pas.
- **Page `/p`**, en `locked` :
  - Une ligne de placement, entre le statut et le lien Discord :
    - titulaire : « Titulaire · Équipe 2, slot 4 · Soutien », où le rôle est celui de sa classe (libellé de la référence) ;
    - banc : « Banc · G1 · jeu. 21:00 » ;
    - « Non placé » si `placement` est absent alors que la candidature est `accepted`.
  - Le créneau, en heure locale, et le nom de salle dans une `CopyChip`.
  - La liste « Ton équipe » (titulaire) ou « Ton groupe » (banc) : `Pseudo[SRV]` en `mono` et le libellé de classe, groupés par équipe pour le banc.
- **Page `/p`**, si `backInPreparation` est vrai : « Le raid est revenu en préparation. » s'affiche, sans placement ni coéquipiers.
- **Région live :** la région `aria-live` existante annonce aussi les changements de placement (la ligne de placement), et rien d'autre.
- **`deploy/README.md` §16 « Vérification de fin d'epic 3 »**, dans le style des §14 et §15. C'est la liste du vrai raid, à cocher par Vincent :
  1. Créer un Ludra et réunir au moins 10 acceptés de deux serveurs, idéalement des deux factions.
  2. Lire la grille et choisir la date.
  3. Verrouiller en deux touchers.
  4. Placer dix titulaires et un banc à la main, puis Enregistrer.
  5. Dans Salle, copier le nom, créer la salle en jeu avec la recette et vérifier que les slots correspondent à la carte.
  6. Coller le message Discord : l'heure s'affiche dans le fuseau de chacun et aucun secret n'apparaît.
  7. Chaque joueur ouvre `/p` et voit sa place, la salle et ses coéquipiers. Un `pending` n'en voit aucun.
  8. Déverrouiller, puis vérifier « revenu en préparation » sur `/p`. Reverrouiller, puis vérifier que la salle n'a pas changé de nom.
  9. Jouer le raid. Personne ne demande comment se connecter.

  Aucune adresse IP, aucun hôte réel ni aucun secret dans ce texte.
- **Locales :** tous les textes sont en fr et en en.

**Never:**
- Montrer le banc aux titulaires.
- Montrer un autre groupe, un `pending`, un `rejected` ou un `withdrawn`.
- Exposer d'autres champs (CP, serveur seul, id, disponibilités).
- Toucher à `PublicEvent` ou à `/e`.
- Ajouter une migration ou un mécanisme de confirmation (epic 5).
- Affirmer à la place de Vincent que le vrai raid a été joué.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Titulaire | `locked`, accepted, G1 Équipe 2 slot 4 | `placement.seat = starter(2,4)`, `roomName`, `startsAt` ; `teammates` = les autres titulaires de l'Équipe 2 de G1, chacun avec seulement `pseudoSrv`, `class` et `party` | — |
| Banc | accepted au banc de G1 | `seat = bench(1)` ; `teammates` = les titulaires des Équipes 1 et 2 de G1 | — |
| Non placé | accepted sans ligne, ou ligne dans un groupe en sommeil | pas de `placement`, pas de `teammates` | — |
| Non accepté | `pending`, `rejected` ou `withdrawn` (une ligne pending forcée en SQL) | pas de `placement`, pas de `teammates` | — |
| Ouvert | `open` après déverrouillage | pas de `placement` ni de `teammates` ; `backInPreparation = true` | — |
| Ouvert jamais verrouillé | `open`, aucun groupe | pas de `backInPreparation` | — |
| Confinement | 2 groupes placés | aucun pseudo de l'autre groupe ni du banc (pour un titulaire) dans le corps | — |

</intent-contract>

## Code Map

- `crates/server/src/app/queries/participant_view.rs` -- `load(conn, …)`, aussi appelée dans la transaction des commandes participant. On y ajoute le placement, les coéquipiers et `backInPreparation`.
- `crates/server/src/infra/groups.rs` -- `list`, `assignments` et `GroupRow` : lecture des groupes et des placements.
- `crates/domain/src/placement.rs` -- logique pure à ajouter pour « coéquipiers visibles », avec ses tests unitaires.
- `crates/server/src/http/dto.rs` -- `ParticipantView` (AD-5 : champ absent plutôt que vide).
- `crates/server/tests/participant.rs` et `tests/organizer_placement.rs` -- aides `candidate`, `lock` et `place` (via SQL ou l'API de placement).
- `web/src/pages/ParticipantPage.tsx` -- statut, région live, Discord, `CopyChip`.
- `web/src/lib/{labels,time}.ts` -- `roleLabel`, `instanceLabel` et `formatSlot`.
- `deploy/README.md` -- §14 et §15, les modèles pour le §16.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/placement.rs` -- `participant_placement` et `teammates`, avec leurs tests unitaires.
- `crates/server/src/app/queries/participant_view.rs`, `http/dto.rs` -- les trois champs.
- `crates/server/tests/participant_placement.rs` -- chaque ligne de la matrice, plus un test qui vérifie que chaque coéquipier a exactement les clés `pseudoSrv`, `class` et `party`.
- `web/src/pages/ParticipantPage.tsx`, `web/src/api/types.ts`, `web/src/locales/*.json` -- l'affichage.
- `web/src/pages/ParticipantPage.test.tsx` -- tests :
  - lignes titulaire, banc et non placé ;
  - nom de salle copiable ;
  - « Ton équipe » et « Ton groupe » ;
  - « revenu en préparation » ;
  - annonce live d'un changement de placement.
- `deploy/README.md` -- §16.

**Acceptance Criteria:**
- Étant donné un raid verrouillé et placé, quand un titulaire ouvre `/p`, alors il voit « Titulaire · Équipe N, slot M · Rôle », le créneau, le nom de salle à copier, et les seuls autres titulaires de son équipe.
- Étant donné le §16 de `deploy/README.md`, quand Vincent monte le vrai raid, alors chaque élément du signal de succès de l'epic 3 a sa case à cocher.

## Spec Change Log

- 2026-11-07 — relecture : la forme `{pseudoSrv, class, party}` de la spec contredisait AD-4 et CAP-6 (« rien d'autre »). Correction : `teammates: [{party, members: [{pseudoSrv, class}]}]` ; `party` devient une structure de regroupement, plus un champ du coéquipier. À GARDER : les règles de visibilité (titulaire : son équipe ; banc : les titulaires de son groupe).

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 22 findings — high 0, medium 4, low 18, false 0, maybe-false 0
- findings:
  - `[medium]` `[patch]` Chaque coéquipier porte `party`, contraire à AD-4 et à CAP-6 (« Pseudo[SRV] et classe, rien d'autre ») — nouvelle forme `teammates: [{party, members: [{pseudoSrv, class}]}]` ; le test HTTP vérifie les clés des membres. Spec Change Log mis à jour.
  - `[low]` `[reject]` Les tests web reposent sur des fixtures, pas sur le serveur réel — règle d'AD-5 (miroir à la main) ; le serveur est testé par HTTP.
  - `[low]` `[patch]` Une annonce de texte identique n'est pas relue — espace insécable alternée ; testé.
  - `[medium]` `[patch]` `backInPreparation` envoyé pour tout statut — seulement pour une candidature `accepted` ; testé.
  - `[medium]` `[patch]` Bandeau « revenu en préparation » côté client pour tout statut — même correctif que `backInPreparation`.
  - `[low]` `[patch]` Annonce de l'id de classe brut quand la référence n'est pas chargée — annonce différée jusqu'au chargement de la référence.
  - `[low]` `[patch]` La case « salle trouvée en jeu par son nom » manque au §16 — ajoutée.
  - `[medium]` `[patch]` `party` contraire à la spine (Blind Hunter) — même correctif que la nouvelle forme.
  - `[low]` `[patch]` Changement de groupe ou de créneau d'un titulaire annoncé à l'identique — même correctif que l'espace insécable.
  - `[low]` `[patch]` Créneau affiché deux fois, jour répété — une seule ligne date · heure.
  - `[low]` `[patch]` Liste de coéquipiers vide sans message — « Personne d'autre n'est placé pour l'instant. »
  - `[low]` `[patch]` Raid verrouillé puis annulé non testé — test HTTP ajouté.
  - `[low]` `[patch]` Garde dupliquée, et paramètres du domaine morts — `load` s'appuie sur `participant_placement`.
  - `[low]` `[reject]` Chaque rafraîchissement charge tout l'événement — au plus 200 lignes, toutes les 10 s ; coût négligeable.
  - `[low]` `[patch]` Le §16 omet sauvegarde, build et `curl` — ajoutés : `x-app-build`, sauvegarde §10, `curl` d'exposition.
  - `[low]` `[reject]` « Non placé » annoncé juste avant la vraie place au verrouillage — état transitoire réel et exact ; il dure jusqu'au placement par l'organisateur.
  - `[low]` `[patch]` Commentaire d'en-tête trop long — replié.
  - `[low]` `[patch]` `backInPreparation` absent de la réponse de la commande de modification, non testé — test ajouté.
  - `[low]` `[patch]` Statut et placement changés au même rafraîchissement non testés — test ajouté.
  - `[low]` `[patch]` La fixture du banc ne distingue pas `groupNumber` de `benchIndex` — fixture `groupNumber: 2`.
  - `[low]` `[reject]` Remise à zéro de la clé de placement au changement de secret non testée — chemin rare, avec plusieurs liens sur un même appareil ; la remise à zéro du statut avait déjà ce trou.
  - `[low]` `[reject]` Choix de la région live (pas d'annonce pour un renommage de salle) — conforme à la spec (« n'annonce que ces changements »).

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh` -- expected: OK ; `grep -nE "([0-9]{1,3}\.){3}[0-9]{1,3}" deploy/README.md` ne montre que des adresses de documentation.

## Auto Run Result

Status: done

**Résumé :**
- `ParticipantView` gagne `placement` (groupe Gn, créneau, nom de salle, place de titulaire ou de banc) et `teammates`.
  - `teammates` est regroupé par équipe, avec seulement `Pseudo[SRV]` et la classe.
  - Un titulaire voit son équipe ; un joueur au banc voit les titulaires de son groupe.
- `backInPreparation` est vrai pour un `accepted` dont l'événement est déverrouillé après un verrouillage.
- `/p` affiche :
  - la ligne de placement ;
  - le créneau en une ligne ;
  - le nom de salle à copier ;
  - « Ton équipe » ou « Ton groupe » ;
  - « Le raid est revenu en préparation. ».
- La région live annonce les changements de placement.
- `deploy/README.md` §16 contient la liste à cocher du vrai raid (13 cases), remise à Vincent.

**Relecture :** 22 remarques, dont 17 corrigées en 12 correctifs et 5 écartées, chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** non.

**Vérification :**
- fmt et clippy `-D warnings` passent ; `REQUIRE_WEB_DIST=1 cargo test` : 367 tests, 0 échec.
- `list-error-codes` passe, tout comme le contrôle des crates réseau, le durcissement et `plan-deploy`.
- Web : tsc et vitest passent (438 tests). Le JS initial pèse 118,7 ko sur 150.

**Risques résiduels :** le vrai raid n'a pas été joué. C'est le §16, à faire par Vincent. L'epic 3 s'arrête à ce point de contrôle.
