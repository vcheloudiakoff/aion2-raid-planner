---
title: 'Salle et message Discord'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '5fad015b1e0f32abc5dcb1f4e2eead983edce7d2'
review_loop_iteration: 0
followup_review_recommended: false
context:
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/placement-et-salle.md'
  - '{project-root}/_bmad-output/specs/spec-epic-3-organiser/stories/3-3-placement-a-la-main.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** l'organisateur a ses groupes et ses placements. Il doit encore tout retaper pour créer la salle en jeu et pour prévenir les joueurs sur Discord (SPEC CAP-4, CAP-5).

**Approach:**
- **Onglet Salle.** Il est chargé à la demande et montre une carte par groupe retenu : nom à copier, recette, slots et banc à copier.
- **« Régénérer le nom ».** Une commande serveur, derrière une confirmation.
- **« Copier le message Discord ».** Le message est construit côté client par une fonction pure, à partir de gabarits des locales, sans aucun secret.

## Boundaries & Constraints

**Always:**

**Régénération du nom**
- **Route.** `POST /api/organizer/room-name`, corps `{groupId}`, droit `OrganizerLink`, classe `write`, corps lu par `FormJson`, aucun id dans l'URL. La réponse est `OrganizerEvent`.
- **Action.** Nouvelle action `EventAction::RegenerateRoom`, avec la même règle que `Place` : permise en `locked`, même si le créneau a commencé. `open` donne `event.not_locked`, `cancelled` donne `event.closed`. Le test exhaustif d'`event_may` est étendu.
- **Groupe visé.** S'il est inconnu, appartient à un autre événement ou est en sommeil : 400 `placement.unknown_group` avec `fields.groupId`.
- **Nouveau nom.**
  - Composé par `compose(template, room_label figé du groupe, room_code())`. `free_room_name` est réutilisé, après être passé en `pub(crate)` ou en helper partagé.
  - Le nom actuel compte comme pris : le nom change toujours.
  - Après 5 collisions : 500, rien n'est écrit.
  - Le nom n'apparaît jamais dans les journaux.
- **Écriture.** Nouvelle fonction `groups::set_room_name`. C'est le seul autre writer du nom (AD-18).

**Onglet Salle**
- Il est en `locked` seulement, comme module `React.lazy` (`components/Room.tsx`), avec un squelette sous `Suspense`. Le JS initial reste sous 150 KB.
- `retainedGroups` passe dans `lib/placement.ts`. GroupList ne garde au plus que la ligne « N groupe(s) en sommeil ».
- **En tête d'onglet :** « Copier le message Discord », le bouton principal, unique.
- **Une carte par groupe retenu,** dans l'ordre des créneaux (« G1 · jeu. 21:00 »), qui contient dans l'ordre :
  - **Le nom de salle,** en `display` dans une puce à copier. `CopyChip` gagne une variante `display`.
  - **La recette, en deux ou trois lignes :**
    1. « Crée une salle de Sanctuaire : {instance} ». Une clé de locale par `kind` d'instance, `sanctuary` ou `expedition` (« Crée une salle d'expédition : {instance} »).
    2. « Nom : {room} ».
    3. « Faction : All », seulement si `factionScope = all`. Sinon, cette ligne est absente.
  - **Équipe 1 puis Équipe 2, slot par slot :**
    - numéro, rôle du titulaire (`roleLabel`) et `Pseudo[SRV]` dans une puce à copier ;
    - un slot vide affiche « libre » en `ink-disabled`.
  - **Le banc, dans l'ordre,** avec chaque `Pseudo[SRV]` à copier. Absent si le banc est vide.
  - **Un menu « … » avec « Régénérer le nom ».**
    - Il ouvre une `ConfirmSheet` : « Le nom change. Celui que tu as posté sur Discord deviendra faux. », avec le bouton danger « Régénérer ».
    - Il passe par `command("roomName", …)`, donc un seul envoi à la fois.
    - `placement.unknown_group` y est traité comme un refus d'état : la feuille se ferme et la vue est relue.

**Message Discord**
- **Construction.** Fonction pure `lib/discord.ts` → `discordMessage(view, reference, lang, origin)`. Elle ne reçoit jamais le secret.
  - Un bloc par groupe retenu, dans l'ordre des créneaux, les blocs séparés par une ligne vide.
  - Gabarits des locales `discord.*`, en fr et en en, dans la langue de l'interface. Libellés d'instance et de rôle tirés de la référence.
- **Contenu d'un bloc :**
  1. Ligne de titre : `{instance} · <t:{unix}:F>`, où `unix = floor(Date.parse(startsAt)/1000)` du créneau du groupe.
  2. « Salle : {room} ».
  3. « Faction : All » si `factionScope = all`.
  4. « Équipe 1 », puis une ligne par slot : `{n}. {Pseudo[SRV]} · {rôle}`, ou « libre ». Puis « Équipe 2 » de la même façon.
  5. « Banc : » suivi des pseudos dans l'ordre, si le banc n'est pas vide.
- **Fin du message :** une seule ligne pour tous les blocs, le lien public `${origin}/e/${event.id}`.
- **Interdits :** jamais `/o/`, `/j/` ou `/p/`, ni aucun secret.
- **Copie.** `navigator.clipboard.writeText`, puis le toast « Copié » pendant 1,5 s. Si le presse-papiers échoue, le message s'affiche dans un `<textarea>` en lecture seule, focalisé et entièrement sélectionné.
- **Sans placement.** Les cartes s'affichent avec des slots « libre », et la copie marche quand même.

**Locales**
- Tous les textes sont en fr et en en, avec le ton *Voice and Tone*. Les clés et les placeholders sont identiques dans les deux langues.

**Never:**
- Une migration.
- Composer le nom côté client.
- Mettre un secret, `/o`, `/j` ou `/p` dans le message.
- Livrer « Demander confirmation » (epic 5).
- Modifier la vue participant (3-5).
- Mettre un id dans l'URL.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Régénérer | `locked`, groupe retenu | 200 ; nouveau nom différent, unique | — |
| Collision | le générateur renvoie le nom courant puis un nom pris, puis un libre | le nom libre est retenu | 5 collisions → 500, rien n'est écrit |
| Groupe invalide | inconnu, d'un autre événement ou en sommeil | rien n'est écrit | 400 `placement.unknown_group` + `fields.groupId` |
| Statut | `open` ; `cancelled` | rien n'est écrit | 409 `event.not_locked` ; 409 `event.closed` |
| Message | 2 groupes placés, portée `all` | 2 blocs dans l'ordre des créneaux, titre avec `<t:…:F>`, salle, « Faction : All », équipes, banc ; lien `/e/<id>` ; aucun secret | — |
| Portée mono-faction | `factionScope = elyos` | recette et message sans ligne « Faction » | — |
| Sans placement | groupe sans titulaire | tous les slots « libre », sans section Banc | — |
| Presse-papiers refusé | `writeText` rejette | le message s'affiche dans une zone sélectionnée | — |

</intent-contract>

## Code Map

- `web/src/pages/OrganizerPage.tsx:70-71,77-94,112,288-328,572-608` -- chargement paresseux de Composition, `STATE_CODES`, `Pending` (à compléter avec `"roomName"`), `LOCKED_TABS`, `origin`, `command`, modèle `Suspense` et placeholder Salle.
- `web/src/components/{GroupList,CopyChip,ConfirmSheet,EventSummary}.tsx` -- `retainedGroups` (à déplacer), la puce de copie (variante `display` à ajouter), la confirmation et `roleLabel`.
- `web/src/api/types.ts:241-270` -- `OrganizerGroup` : `starters` et `bench` triés, `roomName` et `roomLabel`.
- `web/src/lib/placement.ts` -- futur emplacement de `retainedGroups`.
- `web/src/i18n/index.ts` -- `t(key, vars)`. Aucun précédent de `\n` dans les locales : une clé par ligne, jointe en code.
- `crates/server/src/app/commands/lock_event.rs:38,141-159` -- `ROOM_NAME_ATTEMPTS` et `free_room_name`.
- `crates/domain/src/room.rs:127` et `crates/server/src/infra/groups.rs:10-124` -- `compose`, `room_name_taken` et `set_retained`, modèle de `set_room_name`.
- `crates/server/src/app/commands/apply_placement.rs:28-79`, `commands/mod.rs:22-42` -- modèle de commande et `EventWriteError`.
- `crates/domain/src/event.rs:623-635,1467-1495` -- `event_may` et son test exhaustif.
- `crates/server/src/http/{organizer,mod}.rs` -- `post_placement`, `respond` et le routeur `write`.
- `crates/server/tests/organizer_lock.rs:31-62,407-443,883-934` -- le générateur `Scripted` (à partager), les tests de collision et le test de journaux.
- `web/src/pages/OrganizerPage.test.tsx:966-983` -- tests actuels du placeholder Salle, à passer par `Suspense`.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/event.rs` -- `RegenerateRoom` et le test exhaustif.
- `crates/server/src/infra/groups.rs`, `app/commands/regenerate_room_name.rs` (avec le helper de nom partagé) -- la commande.
- `crates/server/src/http/{organizer,mod,dto}.rs` -- la route et le corps `{groupId}`.
- `crates/server/tests/organizer_room.rs` -- chaque ligne serveur de la matrice, avec le générateur scripté partagé dans `tests/common`, plus le quota `write` et des journaux sans le nom.
- `web/src/lib/discord.ts` et son test -- le builder pur. Tests en fr et en en, avec ou sans « Faction », sans placement, avec plusieurs groupes. Aucune présence de `/o/`, `/j/`, `/p/` ni d'un secret.
- `web/src/components/Room.tsx` (paresseux), `CopyChip.tsx`, `GroupList.tsx`, `lib/placement.ts`, `pages/OrganizerPage.tsx`, `api/{organizer,types}.ts` et `locales/*.json` -- l'écran.
- `web/src/**/*.test.tsx` -- tests :
  - cartes dans l'ordre des créneaux ;
  - recette en 2 ou 3 lignes ;
  - puces de copie ;
  - Régénérer avec confirmation, puis le nouveau nom affiché ;
  - refus d'état ;
  - repli du presse-papiers ;
  - test de page : sur `/o/<SECRET>`, l'argument de `writeText` ne contient pas le secret ;
  - chargement à travers `Suspense`.

**Acceptance Criteria:**
- Étant donné un raid verrouillé et placé, quand l'organisateur ouvre Salle et touche « Copier le message Discord », alors le presse-papiers reçoit le message de tous les groupes retenus, avec `<t:UNIX:F>` et le lien `/e/<id>`, et sans aucun secret.
- Étant donné le build, quand `check-budget` s'exécute, alors le JS initial reste sous 150 KB et Salle est dans un chunk séparé.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 28 findings — high 0, medium 4, low 21, false 3, maybe-false 0
- findings:
  - `[low]` `[patch]` La ligne « en sommeil » de Composition n'est pas testée — test ajouté.
  - `[low]` `[patch]` La portée d'`unknown_group` comme refus d'état n'est pas protégée — test : un 400 de placement reste une erreur structurelle.
  - `[false]` `[reject]` Le lien utilise l'origine du navigateur, ce qui pourrait mettre une adresse du réseau local dans le message — le Caddy du NAS coupe tout client autre que le VPS : `/o` n'est joignable que par l'URL publique.
  - `[low]` `[reject]` La structure du message est codée en dur, pas éditable depuis les locales — chaque ligne vient des locales, ce que permet le gabarit (« depuis un gabarit des locales »).
  - `[medium]` `[patch]` Limites de Discord (audit d'intention) — même correctif que la limite de 2 000 caractères.
  - `[low]` `[reject]` Recette et salle non vérifiées en jeu — c'est la vérification manuelle de fin d'epic (3-5).
  - `[medium]` `[patch]` Un brouillon de Composition non enregistré est ignoré par Salle — avis affiché dans Salle ; testé.
  - `[low]` `[reject]` L'ordre des créneaux n'est pas testé côté client — l'ordre est garanti par la requête serveur, que la 3-2 teste.
  - `[false]` `[reject]` Recette en deux lignes et libellé expédition — c'est la règle conditionnelle de CAP-4 et d'EXPERIENCE.
  - `[medium]` `[patch]` Le message peut dépasser 2 000 caractères — bouton de copie par groupe, et note au-delà de 2 000 ; testé.
  - `[medium]` `[patch]` Brouillon ignoré (Blind Hunter) — même correctif que l'avis dans Salle.
  - `[low]` `[patch]` Titulaires indisponibles non signalés dans Salle — marque « pas dispo » sur la carte.
  - `[low]` `[patch]` La zone de repli garde un message périmé — elle est reconstruite depuis la vue ; testé.
  - `[low]` `[patch]` Texte de refus inadapté à la régénération — texte dédié, « Ce groupe n'est plus retenu ».
  - `[low]` `[patch]` La feuille se ferme en silence quand une commande est déjà en cours — bouton désactivé et feuille gardée ouverte.
  - `[low]` `[patch]` Accessibilité du menu « … » — Échap rend le focus au bouton, un clic extérieur ferme le menu.
  - `[low]` `[patch]` `GroupList` mal nommé — renommé `SleepingGroups`.
  - `[low]` `[patch]` Recherches de libellés en double — `lib/labels.ts` partagé.
  - `[low]` `[reject]` « Dans l'ordre des créneaux » non testé (Blind Hunter) — même verdict que plus haut.
  - `[low]` `[patch]` Aucun test sur le `room_label` figé — test ajouté.
  - `[low]` `[patch]` `set_room_name` ignore `rows_affected` — erreur levée si différent de 1.
  - `[false]` `[reject]` Un renommage concurrent donnerait un 500 — le pool d'écriture n'a qu'une connexion et sérialise les transactions, donc aucune course n'est possible.
  - `[low]` `[reject]` Un bloc sans créneau n'a pas d'heure — impossible, la suppression d'un créneau supprime ses groupes en cascade.
  - `[low]` `[patch]` Zone de repli périmée (Edge Case Hunter) — même correctif.
  - `[low]` `[patch]` Pas de nouveau focus au second échec de copie — compteur d'échecs.
  - `[low]` `[patch]` Toast « Copié » et « Copie impossible » affichés ensemble — le toast est effacé en cas d'échec.
  - `[low]` `[patch]` Focus perdu à Échap (Edge Case Hunter) — même correctif que le menu.
  - `[low]` `[patch]` Recette Sanctuaire affichée par défaut sans référence — ligne neutre.

## Design Notes

**Lien public par l'origine du navigateur :** `PUBLIC_BASE_URL` n'est pas dans `AppState`, décision de la 2-1. La SPA compose ses liens avec `location.origin`, qui vaut en production l'URL publique. Le compagnon écrit `PUBLIC_BASE_URL/e/<id>` : le résultat est identique.

**Recette d'expédition :** la référence actuelle ne contient que Ludra (Sanctuaire). La clé `expedition` prépare les instances à venir.

## Verification

**Commands:**
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings` -- expected: aucun avertissement.
- `REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe.
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert ; JS initial sous 150 KB, Salle dans un chunk séparé.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.

## Auto Run Result

Status: done

**Résumé :**
- **Régénérer le nom :** `regenerate_room_name` passe par `POST /api/organizer/room-name` et n'est possible qu'en `locked`. Le nom est recomposé depuis le `room_label` figé du groupe. Le nom actuel compte comme pris, donc le nom change toujours. La logique de nom libre est partagée avec le verrouillage.
- **Onglet Salle :** il est chargé à la demande et montre une carte par groupe retenu. Chaque carte contient le nom de salle à copier (`display`), la recette en 2 ou 3 lignes, les équipes slot par slot et le banc à copier. Le menu « … » propose « Régénérer le nom », avec confirmation.
- **Message Discord :** il est construit par une fonction pure, sans aucun secret, à partir de gabarits `discord.*` en fr et en. Il contient `<t:UNIX:F>` et le lien `/e/`.
  - Une copie par groupe est proposée, avec une note quand le message complet dépasse 2 000 caractères.
  - Un avis signale un brouillon de Composition non enregistré.
  - Si le presse-papiers échoue, le message s'affiche dans une zone sélectionnée.

**Relecture :** 28 remarques, dont 18 corrigées en 13 correctifs et 10 écartées (3 fausses), chacune avec sa raison dans le journal. Rien n'est reporté.

**Nouvelle relecture recommandée :** non. Deux entrées de gravité moyenne ont été corrigées, sans risque non vérifié identifié au-delà de la vérification en jeu, prévue en 3-5.

**Vérification :**
- Serveur : fmt et clippy `-D warnings` passent ; `REQUIRE_WEB_DIST=1 cargo test` donne 352 tests, 0 échec.
- `list-error-codes` passe avec 49 codes ; crates réseau, durcissement et `plan-deploy` passent aussi.
- Web : tsc et vitest passent (428 tests). JS initial à 117,8 ko sur 150, avec Salle dans un chunk séparé.

**Risques résiduels :**
- Le rendu du message dans Discord (Markdown) et la recette n'ont pas été vérifiés en jeu.
