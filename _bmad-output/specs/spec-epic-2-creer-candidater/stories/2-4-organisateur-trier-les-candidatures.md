---
title: 'Organisateur : trier les candidatures'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '7604d07fbb998fd2e7012eba7932977aeac267ef'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/commandes-et-droits.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-3-ma-candidature.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred:
  - summary: >-
      La boucle de rafraîchissement (séquencement, codes terminaux, arrêt) est recopiée dans EventPage, ParticipantPage et OrganizerPage, et des utilitaires communs vivent dans des modules de page.
    evidence: |-
      `OrganizerPage.tsx` reprend `issued`/`applied`, `stopRef`, `reloadRef` et l'intervalle de `ParticipantPage.tsx` ; `errorText` est importé depuis `NewEvent.tsx` et `DeadLink`/`GONE`/`REFRESH_MS` depuis `EventPage.tsx`. Un correctif de séquencement devra être porté à la main dans trois pages ; à factoriser dans un hook `usePolledView` et un module `lib/` commun.
    location: >-
      web/src/pages/OrganizerPage.tsx
    severity: medium
---

<intent-contract>

## Intent

**Problem:** `/o/<secret>` n'est encore qu'une page d'attente. L'organisateur ne voit pas les candidatures et ne peut ni accepter ni refuser, donc un candidat passé par `/e` reste `pending` pour toujours (SPEC CAP-5, partie tri).

**Approach:**
- **Lecture :** une requête `organizer_view` lue avec le seul secret organisateur.
- **Commande :** `set_candidature_status` applique exactement les transitions d'AD-4.
- **Page `/o/:secret` :** une barre d'onglets Candidatures et Réglages. L'onglet Candidatures liste les candidatures, se rafraîchit toutes les 10 s, signale les doublons et permet de copier `Pseudo[SRV]`.

## Boundaries & Constraints

**Always:**
- **Extracteur `OrganizerLink`** (`http/links.rs`), sur le modèle de `JoinLink` et `ParticipantLink`. Il exige `Right::Organizer` et donne l'`event_id` depuis la ligne de lien. La doc du module parle désormais de « trois exceptions ». Erreurs `auth.*`. Un lien dont l'événement a disparu donne `auth.not_found`.
- **Routes** `[ASSUMPTION]`, avec le secret dans `X-Link-Secret` et aucun id dans l'URL :
  - `GET /api/organizer`, classe `read` ;
  - `POST /api/organizer/candidature-status`, corps `{candidatureId, status}` avec `status ∈ {accepted, rejected}`, classe `write`, lu par `FormJson` sans jeton. Toute autre valeur de `status` donne 400 `form.malformed`.
- **`OrganizerEvent`** (`http::dto`, miroir à la main dans `types.ts`) :
  - forme : `{event: PublicEvent, discordUrl?, classBounds: {classId: {min, max}}, candidatures: OrganizerCandidature[]}` ;
  - `OrganizerCandidature` : `{id, pseudo, server, pseudoSrv, class, role, cp, status, origin, availabilities[{slotId, preferred}], duplicate}` ;
  - liste triée par `created_at` puis `id`, sans jamais réordonner ;
  - `slot.hasGroups` est omis : les groupes arrivent en epic 3.
- **`pseudoSrv`** : un seul util pur dans `domain`, `pseudo_srv(reference, pseudo, server)` → `"{pseudo}[{tag}]"`. Le `tag` vient de `servers.toml`. Si le serveur est absent de la référence, l'id du serveur remplace le tag (AD-6). `role` vaut le rôle de la classe, ou l'id de la classe à défaut.
- **`duplicate`** `[ASSUMPTION]` : vrai pour chaque candidature `pending` ou `accepted` dont le couple (pseudo sans casse, serveur) apparaît au moins deux fois parmi les `pending` et `accepted` de l'événement. Toujours faux pour `rejected` et `withdrawn`. Rien n'est déduplicé (AD-13).
- **Transitions** : `domain::candidature::organizer_may`, pure, avec un test exhaustif. Ordre des contrôles :
  1. Événement `cancelled` : 409 `event.closed`.
  2. Candidature `withdrawn` : 409 `candidature.transition_forbidden`.
  3. Transitions permises : `pending → accepted|rejected` et `accepted ↔ rejected`, en `open` comme en `locked`.
  4. Même statut redemandé : 200 sans écriture, et la vue est renvoyée.
- **Candidature d'un autre événement ou inconnue** : 404 `candidature.not_found`, nouveau code, fr et en.
- **Commande** (modèle `withdraw_candidature`) :
  1. transaction ;
  2. relecture de l'événement et de la candidature avec `WHERE id = ? AND event_id = ?` ;
  3. application de la règle ;
  4. écriture de `set_status` avec `updated_at = clock.now()` ;
  5. relecture de la vue dans la transaction, puis commit.
  - Réponse : 200 `OrganizerEvent`.
  - Aucune ligne `assignments` n'existe avant l'epic 3.
- **`errors.candidature.transition_forbidden`** : texte neutre, car il sert désormais aussi à l'organisateur.
- **Page `/o/:secret`** (remplace `OrganizerPending`) :
  - **En-tête :** titre instance + date. Le secret n'apparaît jamais dans `document.title`.
  - **Barre d'onglets** fixe sous le titre, motif `tablist`. Candidatures est sélectionné par défaut. Réglages montre le résumé public (`Summary`) et le lien Discord en lecture seule : ses réglages arrivent en 2-5.
  - **Ligne de candidature :** c'est un bouton.
    - Contenu : pseudo, `[SRV]` avec le point de faction, classe, CP aligné à droite en chiffres tabulaires.
    - Badge de statut si la candidature n'est pas `accepted` : `pending` bleu ; `rejected` et `withdrawn` en `ink-disabled` barré.
    - Badge « doublon » (`danger`) si `duplicate`.
  - **Feuille de candidature :** un toucher sur la ligne l'ouvre.
    - Contenu : une `CopyChip` de `pseudoSrv`, serveur, classe (rôle), CP, origine (« lien direct » ou « lien public »), créneaux cochés avec ☆.
    - Actions permises par AD-4 : « Accepter » (`pending`, `rejected`) et « Refuser » (`pending`, `accepted`). Aucune action sur `withdrawn` ni sur un événement `cancelled`.
    - « Refuser » passe par une étape de confirmation destructive dans la même feuille, jamais par une seconde feuille empilée.
  - **Après une commande :** la vue renvoyée est appliquée. Sur un code d'état (`event.closed`, `candidature.transition_forbidden`, `candidature.not_found`), la page affiche l'erreur, ferme la feuille et relit la vue.
  - **Rafraîchissement :** toutes les 10 s pendant que la page est visible, avec le séquencement des requêtes de 2-3 (une réponse plus ancienne que la vue affichée est ignorée). Il continue pendant qu'une feuille est ouverte, et la feuille relit sa candidature par id. Il s'arrête sur un code terminal et sur `cancelled`.
  - **Nouvelle ligne :** une candidature absente de la vue précédemment appliquée reçoit un liseré or pendant 3 s, mais jamais au premier chargement ni sur la réponse d'une commande. Sous `prefers-reduced-motion`, le liseré est sans transition.
  - **États de la page :**
    - liste vide : « Aucune candidature pour l'instant. » ;
    - événement `cancelled` : bandeau « Raid annulé », liste lisible, aucune action ;
    - lien mort : le message de 2-2, plus « Retour à l'accueil ».
  - **Lien local :** après la première lecture réussie, `saveOrganizerLink(eventId, secret)` est appelé. Cette nouvelle fonction de `links.ts` garde un `join` existant.
- **Locales :** tous les textes sont en fr et en en, avec le ton de *Voice and Tone*. Les clés `organizer.pending.*` sont supprimées.

**Never:**
- Ajouter une migration.
- Créer une table ou un index par `Pseudo[SRV]`.
- Livrer les réglages, l'annulation (2-5), « Copier mon lien » ou le retrait de l'entrée locale (2-6).
- Livrer la grille, les filtres ou le verrouillage (epic 3).
- Fournir une action de masse ou un geste de balayage.
- Mettre un id ou un secret dans une URL d'API.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Lecture | secret organisateur, 3 candidatures | 200 ; `candidatures` dans l'ordre de création ; `pseudoSrv` = `Pseudo[TAG]` ; `discordUrl` présent si l'événement en a un | secret join → 403 `auth.wrong_right` ; inconnu → 404 `auth.not_found` ; absent → 401 `auth.missing` |
| Doublon | `Abc`/s1 `pending` + `abc`/s1 `accepted` + `Abc`/s1 `rejected` | les deux premières `duplicate=true`, la troisième `false` | — |
| Accepter | `pending` → `accepted`, en `open` ou `locked` | 200 ; statut écrit, `updated_at` = now | — |
| Bascule | `accepted` → `rejected` ; `rejected` → `accepted` | 200 | — |
| Même statut | `accepted` → `accepted` | 200, rien d'écrit | — |
| Retirée | `withdrawn` → `accepted` | rien n'est écrit | 409 `candidature.transition_forbidden` |
| Annulé | événement `cancelled` | rien n'est écrit | 409 `event.closed` |
| Autre événement | `candidatureId` d'un autre événement ou inconnu | rien n'est écrit | 404 `candidature.not_found` |
| Statut invalide | `status: "withdrawn"` ou `"pending"` | rien n'est écrit | 400 `form.malformed` |

</intent-contract>

## Code Map

- `crates/server/src/http/links.rs:13-23,147,172-240` -- doc des exceptions, `lookup()`, `JoinLink` et `ParticipantLink`, modèles d'`OrganizerLink`.
- `crates/server/src/http/mod.rs:43-52,77-89` -- groupes `read` et `write`. `RATE_READ` 600/min et `RATE_WRITE` 120/min (`config.rs:179-180`).
- `crates/server/src/http/participant.rs:24-50` -- handler type : `FormJson`, puis `respond()`.
- `crates/server/src/app/queries/participant_view.rs:17-84` -- lecture publique et `load(conn, …)` réutilisable dans une transaction.
- `crates/server/src/app/commands/withdraw_candidature.rs:159-182` -- modèle de commande d'état.
- `crates/server/src/app/queries/public_event.rs:22` -- `build(reference, snapshot)`.
- `crates/domain/src/candidature.rs:23,54,308-351,774-813` -- `CandidatureStatus`, `is_counted`, `participant_may` et son test, modèles d'`organizer_may`.
- `crates/server/src/infra/candidatures.rs:49-69,151` -- `CandidatureRow` (sans `origin` ni `created_at`), `load` et `set_status`. À ajouter : `list(conn, event_id)` trié, et `load_in_event`.
- `crates/server/src/infra/events.rs:83-123,198,227` -- `EventSnapshot`, `load`, `status_and_candidature_count` et `discord_url`.
- `crates/server/src/http/dto.rs:241,304-327,402-427` -- `NewBound`, `PublicEvent` et `ParticipantView`.
- `crates/server/src/http/errors.rs:65-68,156-162` -- `CandidatureTransitionForbidden`, `EventClosed` et `From<ParticipantRefusal>`. Pas de `candidature.not_found`.
- `crates/server/tests/common/candidatures.rs` et `tests/participant.rs:38-102,501-543` -- aides `create`, `nominal` et `token`. Les aides locales `candidate`, `sql`, `set_event_status` et `set_status` sont à déplacer dans `common`. Modèles des tests de quota et de journaux.
- `web/src/pages/ParticipantPage.tsx:42-66,101-256` -- clés de statut, `STATE_CODES`, séquencement `issued`/`applied`, boucle de rafraîchissement, refus d'état.
- `web/src/pages/EventPage.tsx:21-24,159` -- `REFRESH_MS`, `GONE` et `DeadLink`. `NewEvent.tsx:35` porte `errorText`.
- `web/src/components/{EventSummary,Sheet,ConfirmSheet,CopyChip}.tsx` -- `FactionDot`, `roleLabel`, `Summary`, la feuille et la puce de copie.
- `web/src/pages/OrganizerPending.tsx`, `App.tsx:50-51`, `App.test.tsx:150` et `fr.json:63-65` -- tout ceci est remplacé.
- `web/src/api/links.ts:85` et `api/participant.ts` -- `saveOrganizerLinks`, modèle de `api/organizer.ts`.
- `web/src/test/fixtures.ts` -- ajouter une fixture `organizerEvent`.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/candidature.rs` et `crates/domain/src/pseudo_srv.rs` (ou à un endroit équivalent dans `domain`) -- `organizer_may`, `pseudo_srv` et le calcul de `duplicate`, avec leurs tests unitaires.
- `crates/server/src/http/links.rs` -- `OrganizerLink` et sa doc, avec des tests des refus.
- `crates/server/src/infra/candidatures.rs` -- `list` trié avec `origin` et les disponibilités, et `load_in_event`.
- `crates/server/src/app/queries/organizer_view.rs` et `app/commands/set_candidature_status.rs` -- lecture et commande.
- `crates/server/src/http/organizer.rs`, `http/mod.rs`, `http/dto.rs` et `http/errors.rs` -- routes, DTO et `candidature.not_found`.
- `crates/server/tests/organizer.rs` et `tests/common` -- chaque ligne de la matrice, plus les quotas `read` et `write` sur les routes réelles et l'absence de secret dans les journaux.
- `web/src/api/{organizer,links,types}.ts` -- `getOrganizerView`, `setCandidatureStatus` et `saveOrganizerLink`.
- `web/src/pages/OrganizerPage.tsx`, `components/CandidatureRow.tsx` (ou équivalent), `App.tsx` et `index.css` -- l'écran décrit dans *Always*.
- `web/src/locales/fr.json` et `en.json` -- textes et codes.
- `web/src/**/*.test.tsx` -- tests :
  - onglets ;
  - lignes et badges ;
  - doublon ;
  - feuille avec copie de `pseudoSrv` ;
  - « Accepter » ;
  - « Refuser » avec sa confirmation ;
  - aucune action sur `withdrawn` ni sur un événement `cancelled` ;
  - code d'état qui relit la vue ;
  - liseré seulement sur une nouvelle ligne au rafraîchissement ;
  - séquencement des requêtes ;
  - arrêt du rafraîchissement ;
  - état vide ;
  - lien mort ;
  - `saveOrganizerLink` ;
  - secret absent du titre ;
  - routage dans `App`.

**Acceptance Criteria:**
- Étant donné un candidat passé par `/e` (`pending`), quand l'organisateur l'accepte sur `/o`, alors `/api/participant` de ce candidat renvoie `accepted` avec `discordUrl`.
- Étant donné les locales, quand `list-error-codes` est vérifié, alors `candidature.not_found` est traduit en fr et en en.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 26 findings — high 0, medium 4, low 20, false 2, maybe-false 0
- findings:
  - `[false]` `[reject]` « Trier » au sens d'ordonner la liste — tri et filtres sont des motifs de la grille (EXPERIENCE, epic 3). Ici, « trier » veut dire accepter ou refuser, et l'ordre de création respecte l'interdit de déplacer une ligne.
  - `[false]` `[reject]` Règle de doublon « même chaîne `Pseudo[SRV]` » contre « même joueur à départager » — c'est la règle `[ASSUMPTION]` choisie et suivie par EXPERIENCE (« l'organisateur choisit laquelle refuser ») ; la chaîne et le couple (pseudo, serveur) coïncident, le tag étant unique par région.
  - `[low]` `[reject]` Puce de copie dans la feuille et non dans la ligne — EXPERIENCE fait de la ligne un bouton qui ouvre la feuille ; une puce dans un bouton serait un interactif imbriqué.
  - `[low]` `[reject]` Pas de test de bout en bout pour le délai de 10 s — le serveur est testé (la ligne figure dans la lecture suivante) et le client aussi (minuteurs simulés).
  - `[low]` `[reject]` Liseré et barre fixe vérifiés par leurs classes — happy-dom n'a pas de moteur de rendu ; vérification visuelle à faire en 2-6.
  - `[low]` `[reject]` Miroir `types.ts` recopié à la main — règle d'AD-5 ; les clés correspondent aujourd'hui.
  - `[low]` `[reject]` Ajouts hors « partie tri » (`saveOrganizerLink`, texte neutre de `transition_forbidden` sur `/p`) — c'est la parité avec 2-3 demandée par la spec ; le texte est neutre pour les deux pages.
  - `[medium]` `[patch]` Vue organisateur lue sans instantané commun — lecture dans une transaction du pool de lecture.
  - `[low]` `[reject]` `COUNT(*)` jeté, et un statut inchangé prend la connexion d'écriture — coût négligeable.
  - `[low]` `[reject]` `zip` entre lignes et drapeaux de doublon — `duplicates` rend un drapeau par entrée par construction, ce que vérifient les tests du domaine.
  - `[low]` `[reject]` Repli de casse Unicode et NFKC des pseudos — un doublon non signalé coûte peu (simple indication), et le correctif ajouterait une dépendance.
  - `[low]` `[reject]` Le client redécoupe `[SRV]` dans `pseudoSrv` — le découpage par préfixe du pseudo reste correct pour le format unique d'AD-6 ; exposer le tag ajouterait un champ public.
  - `[medium]` `[patch]` `actionError` jamais effacé — effacé à la vue réussie suivante et à l'ouverture d'une feuille ; testé.
  - `[low]` `[patch]` `aria-controls` vers un panneau absent — posé seulement sur l'onglet sélectionné.
  - `[low]` `[patch]` Faction signalée seulement par la couleur dans la ligne — libellé de faction invisible mais lu par les lecteurs d'écran.
  - `[low]` `[patch]` Pause en arrière-plan non testée — test `visibilityState` « hidden » ajouté. Une relecture au retour sur la page n'est pas exigée : la vue se met à jour au plus 10 s après.
  - `[medium]` `[defer]` Boucle de rafraîchissement recopiée dans trois pages, et utilitaires importés depuis des modules de page — motif hérité de 2-2 et 2-3 ; le factoriser dans un hook partagé est un remaniement, pas un correctif.
  - `[low]` `[patch]` Rôle affiché en double quand la classe manque à la référence — rôle masqué quand `role === class`.
  - `[low]` `[patch]` Bascule `accepted ↔ rejected` en `locked` non testée en HTTP — test étendu à `open` et `locked`.
  - `[low]` `[patch]` Ordre des créneaux dans `list` non vérifié — test ajouté, qui échoue sans l'`ORDER BY`.
  - `[low]` `[patch]` Pause quand l'onglet est masqué non testée (Verification Gap) — même correctif que le test `visibilityState`.
  - `[low]` `[patch]` Actions de la feuille en `locked` non testées côté web — test ajouté.
  - `[medium]` `[patch]` Lecture hors instantané (Edge Case Hunter) — même correctif que la transaction de lecture.
  - `[low]` `[reject]` Repli de casse Unicode (Edge Case Hunter) — même verdict que plus haut.
  - `[low]` `[reject]` Secret changé pendant une décision en vol — rare, car il faut naviguer entre deux liens `/o` pendant un POST ; la garde ajouterait de la complexité.
  - `[low]` `[patch]` Focus perdu après une erreur, qui laisse Tab sortir de la feuille — le focus revient dans la feuille ; testé.

## Design Notes

**Id dans le corps, pas dans l'URL :** cette story suit la règle de 2-2 et 2-3, qui n'admet aucun id dans une URL d'API. La commande vérifie que la candidature appartient bien à l'événement du lien.

**Pas d'ETag ici :** une vue de 200 candidatures pèse quelques dizaines de ko toutes les 10 s, ce qui reste supportable. L'ETag pourra s'ajouter quand le besoin sera mesuré.

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

**Résumé :**
- **Lecture :** la requête `organizer_view` passe par `GET /api/organizer`, avec l'extracteur `OrganizerLink`, sans id dans le chemin. La lecture se fait dans une transaction, sur un seul instantané.
- **Commande :** `set_candidature_status` passe par `POST /api/organizer/candidature-status`, corps `{candidatureId, status}`.
  - Transitions d'AD-4 : `pending → accepted|rejected` et `accepted ↔ rejected`, en `open` comme en `locked`.
  - Refus : `event.closed` pour un événement annulé, `transition_forbidden` depuis `withdrawn`.
  - Le même statut redemandé ne déclenche aucune écriture.
  - Nouveau code : `candidature.not_found` (404).
- **Domaine :** `pseudo_srv` compose `Pseudo[TAG]` côté serveur seulement. `duplicates` repère le même pseudo, sans casse, sur le même serveur, parmi les `pending` et `accepted`.
- **Page `/o/:secret` :**
  - onglets Candidatures et Réglages (le résumé, en lecture seule, en attendant 2-5) ;
  - lignes avec badges de statut et de doublon ;
  - feuille avec copie de `Pseudo[SRV]`, « Accepter » et « Refuser » confirmé dans la même feuille ;
  - rafraîchissement toutes les 10 s, séquencé et en pause quand la page est cachée, avec un liseré or de 3 s sur une nouvelle ligne ;
  - états vide, annulé et lien mort ;
  - lien gardé sur l'appareil.

**Fichiers :**
- `crates/domain/src/{candidature,pseudo_srv}.rs` : `organizer_may`, `pseudo_srv` et `duplicates`.
- `crates/server/src/http/{links,organizer,mod,dto,errors}.rs` : `OrganizerLink`, les routes, `OrganizerEvent` et `candidature.not_found`.
- `crates/server/src/app/queries/organizer_view.rs` et `app/commands/set_candidature_status.rs`.
- `crates/server/src/infra/candidatures.rs` : `list` trié, `origin` et `load_in_event`.
- `crates/server/tests/organizer.rs` : 16 tests.
- `tests/common/candidatures.rs` : aides partagées.
- `web/src/pages/OrganizerPage.tsx` et `components/{CandidatureRow,CandidatureSheet}.tsx`.
- `web/src/api/{organizer,links,types}.ts` et les locales.
- Supprimés : `OrganizerPending.tsx` et les clés `organizer.pending.*`.

**Relecture :** 26 remarques, dont 13 corrigées (10 correctifs), 1 reportée (boucle de rafraîchissement à factoriser) et 12 écartées (dont 2 fausses), chacune avec sa raison dans le journal.

**Nouvelle relecture recommandée :** oui. Deux entrées de gravité moyenne ont été corrigées. Le risque non vérifié est l'effacement de `actionError` : il ne doit pas masquer un refus qui arriverait pendant la relecture qu'il déclenche.

**Vérification :**
- fmt et clippy `-D warnings` passent.
- `REQUIRE_WEB_DIST=1 cargo test` : 250 tests, 0 échec.
- `list-error-codes` : 39 codes, présents dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (216 tests). Budget respecté : 100,0 sur 150 ko de JS.
- Aucune adresse IP dans le diff.

**Risques résiduels :**
- La règle de doublon (pseudo sans casse, statuts `pending` et `accepted`) est un `[ASSUMPTION]`.
- Le rendu (barre fixe, liseré) n'a pas été vérifié dans un vrai navigateur ni sur téléphone.
