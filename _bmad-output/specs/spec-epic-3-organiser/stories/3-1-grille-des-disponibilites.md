---
title: 'Grille des disponibilités'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: 'f49795758c6f6d0dc90ce83f7e98a644fea24d72'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-4-organisateur-trier-les-candidatures.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/mockups/key-grille.html'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** l'organisateur ne voit pas qui est disponible à quel créneau. Il ne peut donc pas choisir sa date (SPEC epic 3, CAP-1).

**Approach:**
- **Serveur :** `OrganizerEvent` gagne, pour chaque créneau, le nombre de disponibles parmi les `accepted` et la couverture par classe, calculés côté serveur dans `domain`.
- **Web :** un onglet Disponibilités sur `/o`, construit pour 360 px d'abord. Il affiche soit une matrice, soit une vue par créneau, avec tri, filtres et couverture du créneau sélectionné. C'est un composant pur, alimenté par la boucle de rafraîchissement existante d'`OrganizerPage`.

## Boundaries & Constraints

**Always:**

**Serveur**
- **DTO.** `OrganizerEvent` gagne `slots: OrganizerSlot[]`, dans l'ordre de `event.slots`. Chaque entrée a la forme `{slotId, availableAccepted, coverage: {classId: n}}` :
  - `availableAccepted` compte les candidatures `accepted` qui ont coché ce créneau ;
  - `coverage` compte ces mêmes candidatures par classe. Il a une entrée par classe bornée de l'événement (zéros compris), plus toute classe `accepted` sans borne ;
  - les candidatures `pending`, `rejected` et `withdrawn` ne comptent jamais.
  - Le type est recopié à la main dans `types.ts`. `PublicEvent` ne change pas.
- **Calcul.** Une fonction pure `domain::counts::slot_coverage(slots, bounds, accepted)`, avec ses tests unitaires. Les stories 3-2 et 3-3 la réutiliseront.

**Onglet et choix de la vue**
- **Onglets.** Ordre : Candidatures (par défaut), Disponibilités, Réglages. Les libellés tiennent à 320 px : ils peuvent passer sur deux lignes, mais jamais déborder.
- **Choix de la vue.** La matrice est affichée si l'événement a au plus 4 créneaux (passés compris) **et** si `matchMedia("(min-width: 22.5em)")` est vrai, soit 328 px utiles à 16 px. Sinon, la vue par créneau est affichée.
  - Lecture par `useSyncExternalStore`, comme dans `theme.ts`.
  - Une loupe à 200 % réduit le viewport CSS, ce qui bascule naturellement vers la vue par créneau. Aucun `ResizeObserver`.
- **Sélecteur « Matrice · Par créneau ».** Deux boutons `aria-pressed`, la matrice par défaut quand elle est possible.
  - Quand la matrice est impossible, son bouton est `aria-disabled` et la raison s'affiche : « 4 créneaux au plus » ou « Écran trop étroit ».
  - Le choix de l'utilisateur est gardé : la matrice revient dès qu'elle redevient possible.
- **Fuseau.** Il est indiqué une fois, en tête de l'onglet (`event.timezone`).

**Lignes et créneaux**
- **Lignes.** Seules les candidatures `pending` et `accepted` apparaissent.
  - Filtres Tous / Acceptés / En attente, avec leurs comptes calculés côté client.
  - Le tri est fait une seule fois, à l'ouverture de l'onglet ou au changement de filtre : `accepted` d'abord, puis le nombre de créneaux cochés (décroissant), puis l'ordre de création.
  - Au rafraîchissement, une ligne ne bouge jamais sous le doigt. Les cellules se mettent à jour en place, une nouvelle ligne s'ajoute à la fin avec le liseré or de 2-4, et une ligne qui sort du filtre disparaît.
- **Créneau sélectionné.** Il est partagé par les deux vues. Par défaut, c'est le premier créneau à venir, sinon le dernier. Un créneau passé reste visible en `ink-disabled`.

**Matrice**
- **Structure.** Un vrai `<table>` avec `<caption>`, `<thead>`, `<tbody>` et `<tfoot>`.
- **En-tête de colonne.** `<th scope="col">` contient un bouton `aria-pressed` qui sélectionne le créneau. Il affiche le jour court et l'heure sur deux lignes, avec un nouveau helper `slotParts(iso, lang)` dans `lib/time.ts`.
- **En-tête de ligne.** `<th scope="row">` contient un bouton qui ouvre la `CandidatureSheet` existante par `openSheet`.
  - Contenu : le pseudo tronqué par une ellipse, sur une largeur de 136 px, puis classe · CP en `meta`.
  - Badge « attente » si la candidature est `pending`.
- **Cellule.** 44 × 44 px, non interactive.
  - Disponible : fond `available` et ✓.
  - Non disponible : `surface-sunken` et tiret.
  - Préféré : petite étoile en coin.
  - Texte `sr-only` : « Pseudo, mar. 21:00, disponible » (ou « non disponible »), suivi de « , préféré » le cas échéant.
- **Pied de colonne.** La valeur `availableAccepted`. Celle du créneau sélectionné est en couleur `accent`.

**Vue par créneau**
- Des puces de créneau qui passent à la ligne, à sélection unique, au format « mar. 21:00 · 7 », avec 7 = `availableAccepted`.
- Puis la couverture du créneau sélectionné, puis la liste des candidatures disponibles à ce créneau. Ce sont des boutons qui ouvrent la feuille.
- Puis un repli « Pas dispo (N) », fermé par défaut.

**Couverture et contraintes**
- **Bloc de couverture** (sous la grille, ou en tête de la vue par créneau).
  - Titre : « {créneau} · N dispo parmi les acceptés ».
  - Une ligne par classe, libellée par la référence :
    - sous le minimum : « n / min m » en `danger` (texte seulement) ;
    - au-dessus du maximum : « n / max M » ;
    - sinon : « n / min m » si min > 0, sinon « n ».
- **Largeurs.** Aucune largeur de 320 à 640 px ne défile horizontalement. Entre 320 et 359 px, la vue par créneau s'impose.
- **État `locked`.** La grille est en lecture seule, ce qu'elle est déjà. Aucun bouton Verrouiller ici : c'est la story 3-2.
- **Locales.** Tous les textes sont en fr et en en, et les libellés de jeu viennent de la référence.

**Never:**
- Une nouvelle boucle de rafraîchissement ou une nouvelle dépendance.
- Un `ResizeObserver`.
- Un défilement horizontal.
- Une cellule cliquable ou du glisser-déposer.
- Des comptes de `pending` dans le pied, les puces ou la couverture.
- Des données de fiabilité.
- Une modification de `PublicEvent`.
- Livrer le verrouillage (3-2) ou la composition (3-3).

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| DTO | 2 `accepted` sur le créneau A (Clerc, Templier), 1 `pending` sur A, 1 `rejected` sur A | `slots[A].availableAccepted=2`, `coverage.cleric=1`, `coverage.templar=1`, autres classes bornées à 0 | — |
| Matrice possible | 3 créneaux, viewport ≥ 22.5em | matrice par défaut, sélecteur actif | — |
| Trop de créneaux | 5 créneaux | vue par créneau, bouton matrice `aria-disabled` avec « 4 créneaux au plus » | — |
| Écran étroit | 3 créneaux, viewport < 22.5em | vue par créneau, « Écran trop étroit » | — |
| Filtre | « En attente » | seulement les `pending` ; pied et puces inchangés | — |
| Sous le minimum | Clerc 1, min 2 | « Clerc 1 / min 2 » en `danger` | — |
| Rafraîchissement | nouvelle candidature, statut changé | l'ordre des lignes existantes ne change pas ; la nouvelle ligne est en fin de liste avec le liseré | — |

</intent-contract>

## Code Map

- `crates/server/src/app/queries/organizer_view.rs:45-103` -- `load` a déjà en mémoire `rows` (statut, classe, dispos), `snapshot.bounds` et `snapshot.slots`. Il faut calculer la couverture avant `rows.into_iter()` (L60) et lire les ids de créneaux avant que `public_event::build` ne déplace `snapshot` (L98).
- `crates/server/src/http/dto.rs:335,444-456` -- `PublicSlot`, `OrganizerEvent` et `ClassBound`.
- `crates/domain/src/counts.rs:39-85` -- `public_counts` et `role_of`, avec leurs tests unitaires : le modèle à suivre pour `slot_coverage`.
- `crates/server/tests/organizer.rs:151-152,391-421` -- le test des clés de premier niveau (à mettre à jour) et le modèle de test des disponibilités.
- `web/src/pages/OrganizerPage.tsx:59-61,82,132-260,277-293,342,365-438` -- `TABS`, onglet par défaut, `apply` avec `fresh`, refresh, `command`, clavier de la tablist, style des onglets (`flex-1 px-3`), panneaux et `CandidatureSheet`.
- `web/src/api/types.ts:117,187-219` -- `PublicSlot`, `OrganizerCandidature.availabilities` (seulement les créneaux cochés) et `OrganizerEvent`.
- `web/src/components/{CandidatureRow,CandidatureSheet,EventSummary}.tsx` -- `StatusBadge`, la feuille et les libellés.
- `web/src/lib/time.ts:20,36,43` -- `timeZoneLabel`, `formatSlot` et `formatDay`.
- `web/src/theme.ts:11-20` et `theme.test.ts:7` -- le modèle `matchMedia` avec `useSyncExternalStore`, et son bouchon de test.
- `web/src/test/fixtures.ts:94-97,139-208` -- les créneaux et la fixture `organizerEvent`, à compléter avec `slots`.
- `web/src/pages/OrganizerPage.test.tsx:136-166` -- le test de la tablist, qui attend exactement 2 onglets.
- `web/src/locales/fr.json:75-99,132` -- les clés `organizer.*` et `event.timezone`.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/counts.rs` -- `slot_coverage` et ses tests : statuts exclus, classe sans borne, créneau sans disponibles.
- `crates/server/src/app/queries/organizer_view.rs`, `http/dto.rs` -- `slots: Vec<OrganizerSlot>`.
- `crates/server/tests/organizer.rs` -- clés de premier niveau ; ligne DTO de la matrice, sur le vrai routeur.
- `web/src/api/types.ts`, `web/src/test/fixtures.ts` -- miroir du DTO et fixture.
- `web/src/lib/time.ts` -- `slotParts`, et un hook de largeur (`lib/viewport.ts` ou équivalent).
- `web/src/components/AvailabilityGrid.tsx` (et ses sous-composants) -- la matrice, la vue par créneau, la couverture, les filtres et le sélecteur.
- `web/src/pages/OrganizerPage.tsx` -- le troisième onglet et les libellés qui tiennent à 320 px.
- `web/src/locales/fr.json` et `en.json` -- textes.
- `web/src/**/*.test.tsx` -- chaque ligne de la matrice côté web, plus :
  - sémantique de la table (`caption`, `th scope`, `sr-only` des cellules) ;
  - sélection d'un créneau dans les deux vues ;
  - ouverture de la feuille depuis une ligne ;
  - repli « Pas dispo » ;
  - créneau passé ;
  - tablist à 3 onglets.

**Acceptance Criteria:**
- Étant donné un événement à 3 créneaux et 5 candidatures, quand l'organisateur ouvre Disponibilités sur un écran de 360 px, alors la matrice s'affiche sans défilement horizontal : colonne joueur de 136 px et 3 colonnes de 44 px.
- Étant donné un rafraîchissement qui change le statut d'une ligne visible, quand la nouvelle vue est appliquée, alors aucune autre ligne ne change de position.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 30 findings — high 0, medium 7, low 22, false 1, maybe-false 0
- findings:
  - `[low]` `[reject]` Largeur, zoom à 200 % et absence de défilement vérifiés seulement par les classes — l'implémenteur l'a vérifié en Chromium sans interface à 320, 360, 640 px et à 200 % ; le dépôt n'a pas de banc de rendu.
  - `[low]` `[patch]` L'onglet n'est pas testé en `locked` ni en `cancelled` — test de lecture seule ajouté.
  - `[medium]` `[patch]` Le badge doublon manque dans la grille — `[SRV]` dans la ligne meta et badge « doublon » compact ; testé.
  - `[low]` `[reject]` Accord serveur-client par miroir à la main — c'est la règle d'AD-5.
  - `[medium]` `[patch]` Pied, puces et couverture non vérifiés après un changement de vue — test `update()` avec des `slots` modifiés.
  - `[low]` `[patch]` Créneau sélectionné disparu — repli sur le défaut ; testé par un rafraîchissement.
  - `[low]` `[reject]` Pas de défilement vérifié seulement par les classes (Verification Gap) — même verdict que la première ligne.
  - `[medium]` `[patch]` Doublons indiscernables dans la matrice — même correctif que le badge doublon.
  - `[medium]` `[patch]` Annonce vocale identique pour deux doublons — `pseudoSrv` dans l'annonce. Limite : deux doublons de même casse restent indiscernables à l'oral, alors que le badge les signale à l'écran.
  - `[medium]` `[patch]` La sélection par défaut saute quand un créneau commence — le défaut est figé au premier rendu ; testé.
  - `[low]` `[patch]` Une seule raison affichée quand les deux s'appliquent — les deux sont affichées.
  - `[low]` `[reject]` `addListener` absent avant Safari 14 — navigateurs de 2020, hors cible.
  - `[low]` `[reject]` Barre de défilement de bureau entre 360 et environ 376 px — une fenêtre de bureau aussi étroite est rare, et la vue par créneau reste accessible.
  - `[false]` `[reject]` Une ligne qui sort du filtre fait remonter celles d'en dessous — la spec dit expressément « une ligne qui sort du filtre disparaît » ; les autres gardent leur ordre relatif.
  - `[medium]` `[patch]` La sélection change toute seule (Blind Hunter) — même correctif que le défaut figé.
  - `[low]` `[patch]` Changer d'onglet efface les choix de la grille — vue, filtre et créneau remontés dans `OrganizerPage` ; testé.
  - `[low]` `[patch]` Un créneau passé n'est signalé que par la couleur — « passé » ajouté en `sr-only` et dans l'annonce.
  - `[medium]` `[patch]` Doublons non signalés dans la grille (Blind Hunter) — même correctif que le badge doublon.
  - `[low]` `[patch]` `PendingBadge` recopie le badge de statut — il est construit depuis `STATUS_STYLE.pending`.
  - `[low]` `[patch]` Pas de titre h2 dans le panneau — h2 « Disponibilités » ajouté, cible de focus.
  - `[low]` `[reject]` Colonnes en px contre une règle de largeur en `em` — la règle bascule vers la vue par créneau avant tout débordement de page ; les cellules ne contiennent que ✓ ou un tiret.
  - `[low]` `[reject]` Absence de défilement non vérifiée (Blind Hunter) — même verdict que la première ligne.
  - `[low]` `[reject]` Pas de liseré sur une ligne qui revient dans la grille — le liseré marque les nouvelles candidatures (EXPERIENCE), pas les changements de statut.
  - `[low]` `[patch]` Focus perdu après une décision qui retire la ligne — le focus revient au h2 du panneau ; testé.
  - `[low]` `[patch]` État `locked` non testé (Blind Hunter) — même correctif que le test de lecture seule.
  - `[low]` `[reject]` Couverture vide si un créneau manque de `view.slots` — `slots` et `event.slots` viennent du même instantané, donc l'écart est impossible.
  - `[low]` `[patch]` `withSlots` produit une date invalide au-delà de 9 créneaux — jour complété par `padStart`.
  - `[low]` `[reject]` Ordre des créneaux non vérifié si les deux listes divergent — elles ont une seule source (`snapshot.slots`).
  - `[low]` `[reject]` `useMedia` recrée sa `MediaQueryList` à chaque lecture — coût négligeable.
  - `[low]` `[patch]` Libellés anglais ambigus avec « free » — remplacé par « available ».

## Design Notes

**Pied et puces comptent seulement les `accepted` :** l'organisateur verrouille sur ceux qu'il a déjà retenus. Les `pending` restent visibles dans les lignes pour qu'il puisse trier. Le filtre ne change pas le pied, sinon le même chiffre changerait de sens selon le filtre.

**4 créneaux au plus, contre « jusqu'à 7 sur PC » dans EXPERIENCE (Responsive) :** le SPEC tranche pour 4 ; l'extension à 7 sur PC est reportée.

**Largeur en `em` :** la règle suit à la fois la loupe du navigateur et la taille de police choisie par l'utilisateur, sans détecteur de zoom.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert, JS sous 150 KB.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.

## Auto Run Result

Status: done

**Résumé :**

- **Serveur :** `OrganizerEvent.slots` donne, pour chaque créneau, le nombre de disponibles parmi les `accepted` et leur couverture par classe. Le calcul est fait par `domain::counts::slot_coverage`, qui n'est pas lié à l'HTTP.
- **Web :** l'onglet Disponibilités sur `/o` (`AvailabilityGrid`) propose deux vues :
  - **matrice :** au plus 4 créneaux, sur un écran d'au moins 22,5 em (`matchMedia`), en vrai `<table>` accessible ;
  - **vue par créneau :** des puces, la couverture, les disponibles et un repli « Pas dispo ».
- **Comportement de la grille :**
  - tri unique et filtres Tous, Acceptés ou En attente ;
  - pied et couverture calculés sur les seuls `accepted`, avec un minimum non atteint en `danger` ;
  - créneau sélectionné par défaut figé à l'ouverture ;
  - choix gardés d'un onglet à l'autre ;
  - doublons signalés par le serveur et un badge.

**Relecture :** 30 remarques, dont 17 corrigées en 12 correctifs et 13 écartées (1 fausse), chacune avec sa raison dans le journal. Rien n'est reporté.

**Nouvelle relecture recommandée :** oui. Plusieurs entrées de gravité moyenne ont été corrigées. Le risque non vérifié est le gel du créneau par défaut combiné aux choix remontés dans `OrganizerPage` après un changement de secret.

**Vérification :**
- Serveur : fmt et clippy passent ; `REQUIRE_WEB_DIST=1 cargo test` donne 286 tests, 0 échec.
- `list-error-codes` passe : 41 codes.
- Le contrôle des crates réseau passe.
- Web : tsc et vitest passent (335 tests). Budget : 110,7 sur 150 ko de JS.
- Rendu vérifié par l'implémenteur en Chromium sans interface à 320, 360 et 640 px et à 200 %.

**Risques résiduels :**
- Rien n'a été vérifié sur un vrai téléphone.
- L'extension à 7 créneaux sur PC (EXPERIENCE) est reportée : le SPEC tranche pour 4.
