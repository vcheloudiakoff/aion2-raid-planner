---
title: 'Liste des acceptés sur /p'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '404b43591dcd5636f0e297024a19c0dd6a5de513'
review_loop_iteration: 0
followup_review_recommended: false
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/stories/3-5-placement-cote-participant-et-vrai-raid.md'
warnings: []
deferred:
  - summary: >-
      EXPERIENCE.md (Ma candidature) ne décrit pas la section « Acceptés » (repliable, ouverte sous 15, marque « toi »).
    evidence: |-
      À reporter par bmad-ux, l'unique writer du document.
    location: >-
      _bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
    severity: low
---

<intent-contract>

## Intent

**Problem:** sur `/p`, un joueur accepté ne voit que ses coéquipiers, et seulement après le verrouillage. Vincent veut que les acceptés voient la liste de tous les acceptés du raid, sans que la page publique `/e` montre de nom (décision du 2026-11-07, AD-4 et CAP-6 amendés).

**Approach:** `ParticipantView` reçoit un nouveau champ `accepted`, et `/p` affiche une section « Acceptés ».

## Boundaries & Constraints

**Always:**
- **Champ `accepted`** de `ParticipantView` : `[{pseudoSrv, class}]`, avec exactement ces deux clés.
  - Il est présent seulement si la candidature lue est `accepted`, quel que soit le statut de l'événement. Sinon la clé est absente (AD-5).
  - Il liste toutes les candidatures `accepted` de l'événement, la candidature lue comprise, dans l'ordre de création.
  - La règle est une fonction pure dans `domain`, avec ses tests.
- **Section « Acceptés (N) »** sur `/p`, sous le placement et les coéquipiers : `Pseudo[SRV]` en `mono` et libellé de classe tiré de la référence.
  - La ligne de la candidature lue est marquée « toi ».
  - La section est repliable et ouverte par défaut quand il y a moins de 15 acceptés.
- **Recopie** dans `types.ts`, à la main. Textes en fr et en.

**Never:**
- Toucher à `PublicEvent` ni à `/e`.
- Exposer un `pending`, un `rejected` ou un `withdrawn`.
- Exposer un autre champ : CP, statut, id, disponibilités, placement d'autrui.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Accepté | 3 `accepted` (lui compris), 1 `pending`, 1 `rejected` | `accepted` contient les 3, chacun avec seulement `pseudoSrv` et `class` | — |
| Non accepté | la candidature lue est `pending`, `rejected` ou `withdrawn` | aucune clé `accepted` | — |
| Verrouillé | `locked`, candidature acceptée | `accepted` présent, en plus de `placement` et `teammates` | — |
| Public | `GET /api/events/{id}` | aucun pseudo dans le corps | — |

</intent-contract>

## Code Map

- `crates/server/src/app/queries/participant_view.rs` -- `load`, qui lit déjà toutes les candidatures pour les coéquipiers.
- `crates/domain/src/placement.rs` -- l'emplacement de la fonction pure.
- `crates/server/src/http/dto.rs` -- `ParticipantView`.
- `crates/server/tests/participant_placement.rs` -- les tests HTTP voisins.
- `web/src/components/MyPlacement.tsx` et `web/src/pages/ParticipantPage.tsx` -- l'affichage.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/placement.rs` : `accepted_list`, avec ses tests.
- `participant_view.rs` et `dto.rs` : ajout du champ.
- `crates/server/tests/participant_placement.rs` : chaque ligne de la matrice, avec la vérification que chaque élément a exactement les clés `["class", "pseudoSrv"]`.
- `web/src/components/AcceptedList.tsx` (ou équivalent), `ParticipantPage.tsx`, `types.ts` et les locales : la section, testée dans `ParticipantPage.test.tsx` (présente si accepté, absente sinon, marque « toi »).

**Acceptance Criteria:**
- Étant donné un raid avec 3 acceptés et 1 candidat en attente, quand un accepté ouvre `/p`, alors il voit les 3 acceptés et pas le candidat en attente ; et quand le candidat en attente ouvre son `/p`, il ne voit aucune liste.

## Spec Change Log

- 2026-11-07 — relecture : la marque « toi » devinée côté client échouait sur les doublons. Correction : clé renommée `acceptedRoster` (trop proche de `status: "accepted"`), et position du lecteur envoyée par le serveur dans `acceptedRosterSelf`. Les éléments gardent exactement `pseudoSrv` et `class`. Ordre de la liste départagé par `rowid`.

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 3 couches (Blind Hunter, Edge Case Hunter, Verification Gap), 19 constats.
  - 2 de gravité moyenne, corrigés :
    - état replié décidé au montage plutôt qu'à l'apparition de la liste ;
    - marque « toi » devinée côté client.
  - 11 mineurs, corrigés :
    - égalités de `created_at` ;
    - commentaire faux ;
    - module mal placé ;
    - clé ambiguë ;
    - test public trop faible ;
    - test verrouillé trop faible ;
    - réponses des commandes ;
    - repli de libellé de classe ;
    - vue `accepted` sans liste ;
    - doublon ;
    - passage d'en attente à accepté.
  - 3 écartés :
    - liste conservée sur un raid annulé, conforme à « quel que soit le statut de l'événement » ;
    - requête trop large, coût négligeable ;
    - réannonce, conforme à CAP-6.
  - 1 reporté : EXPERIENCE.md à mettre à jour par `bmad-ux`.
## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.

## Auto Run Result

Status: done

Les acceptés voient sur `/p` la liste « Acceptés (N) » de tous les acceptés de l'événement, avec seulement `Pseudo[SRV]` et la classe. Leur propre ligne est marquée « toi » à partir de la position envoyée par le serveur. La liste est repliée à partir de 15. La page publique `/e` reste sans aucun nom. Vérification : 375 tests serveur et 445 tests web passent, JS initial à 119,0 ko.
