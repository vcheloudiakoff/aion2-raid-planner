---
title: 'Ma candidature'
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '6abb1d6eb1287a5b5dc9a6b1ee8661bceb099064'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/commandes-et-droits.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-2-page-publique-et-candidature.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** après sa candidature, un joueur arrive sur une page `/p` provisoire. Il ne peut ni relire son statut sur un autre appareil, ni modifier ses créneaux, ni se retirer (SPEC CAP-4).

**Approach:**
- **Lecture :** la requête `participant_view` lit la candidature avec le seul secret participant.
- **Commandes :** `update_candidature` et `withdraw_candidature`, en classe `write`, sans jeton de formulaire (AD-7).
- **Page `/p/:secret` :** elle affiche le lien personnel, le statut, la lecture de la candidature, sa modification et le retrait avec confirmation destructive.

## Boundaries & Constraints

**Always:**
- **Extracteur `ParticipantLink`** (`http/links.rs`), sur le modèle de `JoinLink` : il exige `Right::Participant` et donne `event_id` et `candidature_id` depuis la ligne de lien. Il est sans `event_id` dans le chemin, parce que `/p` ne connaît que le secret. La doc du module parle désormais de « deux exceptions ». Ses erreurs sont `auth.*`, comme pour `LinkSecret`.
- **Routes** `[ASSUMPTION]`, secret dans `X-Link-Secret` :
  - `GET /api/participant`, classe `read` ;
  - `POST /api/participant/candidature` (modification), dans un nouveau groupe `write` (`RouteClass::Write`) ;
  - `POST /api/participant/withdrawal`, même groupe.
  - Les deux POST lisent leur corps par `FormJson<T>` : pas de jeton, pas de honeypot, pas de disjoncteur.
- **`ParticipantView`** (`http::dto`, miroir à la main dans `types.ts`) :
  - Contenu : `{eventId, candidatureId, pseudo, server, class, cp, status, availabilities[{slotId, preferred}], event: PublicEvent, discordUrl?}`.
  - `discordUrl` est présent seulement si `status = accepted` et que l'événement a un lien. Sinon la clé est absente.
  - La vue ne contient jamais d'autre candidature ni d'autre nom.
- **Règles d'état** (AD-4), vérifiées dans la transaction après relecture de l'agrégat, dans l'ordre :
  1. Événement `cancelled` : modification et retrait donnent 409 `event.closed`.
  2. Candidature `withdrawn` : modification et retrait donnent 409 `candidature.withdrawn`.
  3. Modification seule :
     - événement `locked` : 409 `event.locked` ;
     - candidature `pending`, `accepted` ou `rejected` : permise.
  4. Retrait seul :
     - candidature `pending` ou `accepted` : elle passe à `withdrawn`, en `open` comme en `locked` ;
     - candidature `rejected` : 409 `candidature.transition_forbidden`.
- **Modification :**
  - Corps : même forme que `SubmitCandidature`, sans enveloppe (`pseudo`, `server`, `class`, `cp`, `availabilities`). Il remplace tout le contenu.
  - `status` et `origin` ne changent jamais. `updated_at` reçoit `clock.now()`.
  - La validation réutilise `validate_candidature`, avec un mode modification :
    - `availabilities` ne vise que des créneaux à venir ; une disponibilité d'un créneau passé reste telle quelle en base ;
    - la règle « au moins une disponibilité » compte celles conservées et les nouvelles ;
    - les contrôles `enabled` du serveur et de la classe ne s'appliquent que si la valeur change (AD-6).
  - La commande relit l'événement, ses créneaux et la candidature dans la transaction, puis remplace les `availabilities` à venir. Une disponibilité vers un créneau disparu donne `candidature.slot_unknown`, jamais un 500. Cela règle le report de 2-2 pour ce chemin.
  - Réponse : 200 avec `ParticipantView`.
- **Retrait :** corps vide `{}`. Réponse 200 avec `ParticipantView`. `updated_at` reçoit `clock.now()`.
- **Nouveaux codes :** `event.locked` (409), `candidature.withdrawn` (409) et `candidature.transition_forbidden` (409), avec fr et en.
- **Page `/p/:secret`** (remplace `ParticipantPending`), de haut en bas :
  1. **Lien personnel :** « Ton lien personnel » avec une `CopyChip` du lien complet, « Partager » si Web Share existe, et l'avertissement de 2-2.
  2. **Statut :** un badge, avec ces libellés :

     | `status` | Libellé | Style du badge |
     |---|---|---|
     | `pending` | « En attente de l'organisateur » | bleu |
     | `accepted` | « Acceptée » | — |
     | `rejected` | « Refusée » | `ink-disabled`, barré |
     | `withdrawn` | « Retirée » | `ink-disabled`, barré |

     Un `aria-live` annonce seulement les changements de statut.
  3. **Lien Discord :** si `discordUrl` est présent, un lien externe « Rejoindre le Discord » (`rel="noopener noreferrer"`).
  4. **Résumé :** le résumé public de l'événement (composant de 2-2).
  5. **Ta candidature :** pseudo, serveur (faction), classe (rôle), CP, créneaux cochés et ☆, en lecture.
  6. **Actions :**
     - « Modifier », bouton secondaire. Il est visible si la modification est permise, sinon il est remplacé par le texte de la raison.
     - « Me retirer », bouton secondaire de danger. Il est visible si `pending` ou `accepted` et que l'événement n'est pas `cancelled`.
- **Modification côté web :**
  - « Modifier » ouvre le formulaire de candidature de 2-2, extrait en composant réutilisable, prérempli, sans jeton ni honeypot.
  - Bouton principal unique « Enregistrer ». « Annuler » revient à la lecture.
  - Erreurs : `fields` affichés sous les champs, saisie conservée.
  - Un succès revient en lecture avec la vue renvoyée.
- **Retrait côté web :**
  - Il passe par une feuille de confirmation `ConfirmSheet`, extraite de `SettingsSheet` : dialogue, focus piégé, Échap.
  - Texte : « Ta candidature sera retirée. Pour revenir, il faudra candidater à nouveau. »
  - Boutons « Me retirer » (danger) et « Annuler ».
- **États de la page :**
  - `withdrawn` : lecture seule, avec un lien « Candidater à nouveau » vers `/e/<eventId>`.
  - Événement `cancelled` : bandeau « Raid annulé », lecture seule, aucune action.
  - Événement `locked` : « Le raid est verrouillé : tu ne peux plus modifier, seulement te retirer. »
  - Lien inconnu ou mauvais droit : le message unique de 2-2, plus « Retour à l'accueil ».
- **Rafraîchissement :**
  - silencieux toutes les 10 s, jamais pendant une modification en cours ;
  - il s'arrête sur un code terminal, sur `cancelled` et sur `withdrawn` ;
  - l'état de navigation de 2-2 sert au premier rendu.
- **Liens locaux :**
  - après la première lecture réussie, `saveParticipantLink(eventId, candidatureId, secret)` est appelé : un autre appareil retient ainsi le lien ;
  - si l'appel renvoie false, la page affiche l'avertissement « copie-le maintenant » ;
  - le secret n'est jamais dans `document.title`.
- **Locales :** tous les textes sont en fr et en en, ton *Voice and Tone*.

**Never:**
- Ajouter une migration : `updated_at` suffit.
- Laisser l'organisateur changer un statut (story 2-4).
- Gérer le placement, la salle ou la confirmation (epics 3 et 5).
- Retirer l'entrée locale sur un lien invalide, ou ajouter « Copier mon lien » ailleurs que sur `/p` (story 2-6).
- Mettre un id ou un secret dans une URL d'API.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Lecture | secret participant, candidature `pending` | 200 `ParticipantView`, sans `discordUrl` ni autre candidature | — |
| Lecture acceptée | `accepted`, événement avec Discord | `discordUrl` présent | — |
| Mauvais droit | secret join ou organisateur sur `/api/participant` | — | 403 `auth.wrong_right` ; inconnu → 404 `auth.not_found` ; absent → 401 `auth.missing` |
| Modification | `open`, `accepted`, nouveaux créneaux et CP | 200 ; contenu remplacé, statut `accepted` inchangé, `updated_at` = now | — |
| Modification verrouillée | événement `locked` | rien n'est écrit | 409 `event.locked` |
| Modification invalide | serveur hors région ; aucun créneau | rien n'est écrit | 400 avec `fields`, codes de 2-2 |
| Créneau passé conservé | une dispo sur un créneau passé + modification des créneaux à venir | la dispo passée reste en base | — |
| Retrait | `pending` ou `accepted`, en `open` ou en `locked` | 200 ; `withdrawn` | — |
| Retrait interdit | `rejected` ; déjà `withdrawn` | rien n'est écrit | 409 `candidature.transition_forbidden` ; 409 `candidature.withdrawn` |
| Événement annulé | modification ou retrait | rien n'est écrit | 409 `event.closed` |

</intent-contract>

## Code Map

- `crates/server/src/http/links.rs:19-22,146,171-197` -- doc « seule exception », `lookup()` partagé et `JoinLink`, qui sert de modèle à `ParticipantLink`.
- `crates/server/src/http/mod.rs:42-80` -- groupes `read`, `create` et `candidature` ; il faut ajouter un groupe `write`. `RouteClass::Write` est à `limits.rs:33,105` et `RATE_WRITE` à `config.rs:179`.
- `crates/server/src/http/form_token.rs` -- `FormJson<T>` sert pour les corps sans jeton.
- `crates/server/src/http/errors.rs:52-89,190` -- `codes!{}` et `Problem::fields` générique ; `event.closed` existe déjà.
- `crates/server/src/http/dto.rs:309-387` -- `PublicEvent`, `SubmitCandidature`, `AvailabilityInput` et `CreatedCandidature`.
- `crates/server/src/app/queries/public_event.rs:12,58` -- lecture du `PublicEvent`, à imbriquer dans `ParticipantView`.
- `crates/server/src/app/commands/submit_candidature.rs:39-110` -- modèle de commande ; `context()` (L83) construit `EventContext`.
- `crates/server/src/infra/events.rs:83,123` -- `EventSnapshot` et `load` ; pas de `discord_url`, donc une lecture dédiée est à ajouter.
- `crates/server/src/infra/candidatures.rs:9` -- seulement `insert` ; ajouter la lecture, le remplacement des dispos à venir, le statut et `updated_at`.
- `crates/domain/src/candidature.rs:23,146,163-180,209` -- `CandidatureStatus` et `validate_candidature` ; les règles `enabled` et `slot.in_past` sont à adapter au mode modification.
- `crates/server/tests/candidature_submit.rs:62-150` et `tests/public_event.rs:61` -- aides de test (`setup`, `create`, `nominal`, `assert_fields`, `insert_candidature`), à déplacer dans `tests/common` si elles sont partagées. `tests/common/mod.rs:91,168` : `send_with` et `post_json_with`.
- `web/src/pages/EventPage.tsx:21-104,158-242` -- refresh, `GONE`, `Summary`, `FactionDot`, `roleLabel` et `CandidatureForm`, qui n'est pas exporté : à extraire.
- `web/src/pages/ParticipantPending.tsx` -- à remplacer ; `ParticipantState` vient de la navigation.
- `web/src/components/SettingsSheet.tsx` -- dialogue et focus piégé ; base de `ConfirmSheet`.
- `web/src/api/{candidatures,client,links,types}.ts` -- `getJson` et `postJson` avec `linkSecret`, `saveParticipantLink:94` et `CandidatureStatus:144`.
- `web/src/locales/fr.json:72,100-107` -- clés `participant.*` et `event.cancelled`.

## Tasks & Acceptance

**Execution:**
- `crates/domain/src/candidature.rs` -- mode modification de la validation (créneaux à venir, dispos passées conservées, `enabled` seulement sur une valeur changée) et règles de transition du participant, avec tests unitaires.
- `crates/server/src/http/links.rs` -- `ParticipantLink` et sa doc, avec tests unitaires des refus.
- `crates/server/src/infra/candidatures.rs` et `infra/events.rs` -- lecture de la candidature et du `discord_url`, remplacement des dispos à venir, statut.
- `crates/server/src/app/queries/participant_view.rs`, `app/commands/{update_candidature,withdraw_candidature}.rs` -- AD-5 et AD-16.
- `crates/server/src/http/participant.rs`, `http/mod.rs` (groupe `write`), `http/dto.rs` et `http/errors.rs` -- routes, DTO et 3 codes.
- `crates/server/tests/participant.rs` -- chaque ligne de la matrice côté serveur ; un test vérifie que la vue ne contient aucune autre candidature ; un test vérifie le quota `write` sur la route réelle ; un test vérifie que les journaux ne contiennent ni secret ni pseudo.
- `web/src/components/{CandidatureForm,ConfirmSheet}.tsx` -- extraits de `EventPage` et de `SettingsSheet`, sans régression des tests de 2-2.
- `web/src/pages/ParticipantPage.tsx`, `App.tsx` et `api/participant.ts` -- page de l'*Always*, en remplacement de `ParticipantPending`.
- `web/src/locales/fr.json` et `en.json` -- textes et `errors.<code>`.
- `web/src/**/*.test.tsx` -- tests web :
  - badges des 4 statuts ;
  - Discord seulement si `accepted` ;
  - modification préremplie qui envoie le secret en en-tête, puis retour en lecture ;
  - `locked` : modification masquée et retrait visible ;
  - feuille de confirmation avant le retrait ;
  - `withdrawn` : lecture seule et lien vers `/e` ;
  - `cancelled` : lecture seule ;
  - arrêt du rafraîchissement, et aucun rafraîchissement pendant une modification ;
  - `saveParticipantLink` à la lecture, avec l'avertissement si le stockage est absent ;
  - secret absent du titre ;
  - routage dans `App`.

**Acceptance Criteria:**
- Étant donné un événement avec plusieurs candidatures, quand un participant lit `/api/participant`, alors le corps ne contient que sa propre candidature et le `PublicEvent` sans nom.
- Étant donné une candidature `rejected`, quand le participant la modifie en `open`, alors le contenu change et le statut reste `rejected`.
- Étant donné les locales, quand `list-error-codes` est vérifié, alors les 3 nouveaux codes sont traduits en fr et en en.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 32 findings — high 0, medium 9, low 18, false 5, maybe-false 0
- findings:
  - `[false]` `[reject]` Un `rejected` peut modifier sa candidature — choix voulu : AD-4 rattache la mutabilité à l'événement `open` (Design Notes).
  - `[false]` `[reject]` Retrait permis en `locked` — c'est AD-4 : « en `locked`, le participant ne peut que répondre à une confirmation ou se retirer ».
  - `[low]` `[reject]` Lien Discord visible sur un raid annulé — un participant accepté garde son contact avec l'organisateur ; « lecture seule » veut dire « sans action ».
  - `[false]` `[reject]` Retrait refusé à un `rejected` — c'est AD-4 : le retrait n'existe que depuis `pending` ou `accepted`.
  - `[low]` `[reject]` Le titre répète le libellé de la puce — l'ordre exigé (lien puis statut) est tenu ; c'est cosmétique.
  - `[false]` `[reject]` Fonctions au-delà de l'intention littérale — elles viennent de la spec et de la spine (AD-5, AD-6, AD-7).
  - `[low]` `[reject]` États `accepted`, `locked` et `cancelled` forcés en SQL, sans test de bout en bout — les commandes qui y mènent arrivent en 2-4 et 2-5 ; les routes sont testées sur le vrai routeur.
  - `[false]` `[reject]` Confirmation destructive non imposée par le serveur — c'est un motif d'interface (EXPERIENCE) ; le serveur exige déjà le secret participant.
  - `[medium]` `[patch]` Une lecture périmée écrase la vue après un retrait — séquence de requêtes ajoutée ; la réponse plus ancienne que la vue est ignorée, et c'est testé.
  - `[low]` `[patch]` Une lecture périmée écrase la vue après un enregistrement — même correctif que la séquence.
  - `[medium]` `[patch]` Un 409 d'état laisse le formulaire ouvert avec le rafraîchissement en pause — la page sort de la modification, ferme la feuille et relit la vue ; testé.
  - `[medium]` `[patch]` Un serveur ou une classe absent de la référence bloque l'enregistrement — une option synthétique (libellé = id) est ajoutée ; testé.
  - `[low]` `[patch]` Le focus revient sur un bouton détaché — le focus ne va au déclencheur que s'il est encore dans la page, sinon à `fallbackFocus`.
  - `[low]` `[patch]` Changement de secret pendant une modification — la modification, la feuille et les erreurs sont réinitialisées.
  - `[medium]` `[patch]` Classe désactivée non testée — test `c_off` ajouté.
  - `[low]` `[patch]` Échec du premier chargement non vérifié — l'alerte est vérifiée, puis sa disparition après une lecture réussie.
  - `[low]` `[patch]` Feuille de retrait occupée non testée — test : Échap n'a pas d'effet et le bouton est désactivé pendant l'envoi.
  - `[medium]` `[patch]` Serveur retiré de la référence (Verification Gap) — même correctif que l'option synthétique.
  - `[low]` `[patch]` Focus détaché (Verification Gap) — même correctif que le focus.
  - `[medium]` `[patch]` Serveur ou classe absent de la référence (Blind Hunter) — même correctif que l'option synthétique.
  - `[medium]` `[patch]` 409 pendant une modification ou un retrait — même correctif que la sortie de modification.
  - `[low]` `[patch]` Focus perdu après retrait, enregistrement ou annulation — le focus revient sur « Modifier » ou sur le titre.
  - `[low]` `[reject]` Les candidatures retirées comptent dans le plafond de 200 — règle voulue en 2-2 (« tous statuts confondus », contre l'abus) ; le cas n'arrive qu'à 200 lignes.
  - `[low]` `[reject]` Requête de comptage utilisée pour lire le statut — coût négligeable ; c'est cosmétique.
  - `[low]` `[reject]` « Candidater à nouveau » renvoie un candidat direct vers `/e` — c'est la spec : un retrait est terminal et un retour passe par une nouvelle candidature publique.
  - `[low]` `[patch]` `locked` + `rejected` affiche une clé `errors.*` sans test — clé `participant.lockedNoWithdraw` ajoutée, avec un test.
  - `[medium]` `[patch]` Tests HTTP manquants — 3 tests ajoutés : valeur désactivée inchangée, ordre `locked` + `withdrawn`, créneau supprimé donnant 400 `slot_unknown`.
  - `[medium]` `[patch]` `errors.event.closed` parle de candidatures — reformulé : « Ce raid est annulé : plus aucun changement n'est possible. »
  - `[low]` `[reject]` `Withdrawal {}` accepte des champs en trop — sans effet, le corps est ignoré.
  - `[low]` `[reject]` Indentation du honeypot — aucun formateur web en CI ; c'est cosmétique.
  - `[low]` `[reject]` Double lecture de la ligne `events` — une requête par clé primaire, coût négligeable.
  - `[low]` `[patch]` État de navigation périmé après un rechargement — il est lu une fois puis effacé de l'historique (`replace`, `state: null`).

## Design Notes

**Pas de jeton de formulaire :** AD-7 ne l'exige que pour `create` et `candidature`. Une modification exige déjà le secret participant, qui ne se devine pas.

**Le rejeté peut modifier :** AD-4 rattache la mutabilité du contenu à l'événement (`open`), pas au statut de la candidature. Il donne au participant la propriété de son contenu. Le retrait, lui, n'existe qu'à partir de `pending` et `accepted`.

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
- **Lecture :** la requête `participant_view` passe par `GET /api/participant`. Le nouvel extracteur `ParticipantLink` lit le secret, sans id dans le chemin.
- **Modification :** `update_candidature` passe par `POST /api/participant/candidature`. Elle remplace tout le contenu sans toucher au statut. Elle n'est possible qu'en `open`, et un `rejected` peut aussi modifier. Les disponibilités sur des créneaux passés sont conservées. Le contrôle `enabled` ne s'applique qu'à une valeur changée.
- **Retrait :** `withdraw_candidature` passe par `POST /api/participant/withdrawal`. Il n'est possible que depuis `pending` ou `accepted`, en `open` comme en `locked`.
- **Limiteur :** ces routes forment un nouveau groupe `write`, sans jeton de formulaire.
- **Erreurs :** trois codes 409 sont ajoutés, `event.locked`, `candidature.withdrawn` et `candidature.transition_forbidden`.
- **Page `/p/:secret` :** elle affiche le lien personnel, le statut annoncé par `aria-live`, le Discord si `accepted`, le résumé et la candidature.
  - On peut la modifier depuis cette page, ou se retirer après une feuille de confirmation.
  - Les états retiré, annulé et verrouillé sont gérés.
  - La page se rafraîchit toutes les 10 s et le lien est gardé sur l'appareil à la première lecture.

**Fichiers :**
- `crates/domain/src/candidature.rs` : `validate_update` et `participant_may`.
- `crates/server/src/http/links.rs` : `ParticipantLink`.
- `crates/server/src/http/{participant,mod,dto,errors}.rs` : les routes, le groupe `write`, `ParticipantView` et les 3 codes.
- `crates/server/src/app/queries/participant_view.rs` et `app/commands/{update,withdraw}_candidature.rs` : la lecture et les deux commandes.
- `crates/server/src/infra/{candidatures,events}.rs` : la lecture de la candidature et du Discord, le remplacement des dispos à venir et le statut.
- `crates/server/tests/participant.rs` (16 tests) et `tests/common/candidatures.rs` (aides partagées).
- `web/src/components/{CandidatureForm,ConfirmSheet,Sheet,EventSummary}.tsx` : composants extraits.
- `web/src/pages/ParticipantPage.tsx`, `api/participant.ts`, `App.tsx` et les locales.

**Relecture :** 32 remarques.
- 21 corrigées, réparties en 11 correctifs.
- 11 écartées, dont 5 fausses, chacune avec sa raison dans le journal.
- Rien n'est reporté.

**Nouvelle relecture recommandée :** oui. Neuf entrées de gravité moyenne ont été corrigées. Le risque non vérifié est le séquencement des requêtes de `ParticipantPage` : il ne doit pas ignorer une lecture légitime après un 409 suivi d'une relecture.

**Vérification :**
- fmt et clippy `-D warnings` passent, ainsi que `REQUIRE_WEB_DIST=1 cargo test` (228 tests, 0 échec).
- `list-error-codes` : 38 codes, présents dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (179 tests). Le budget est respecté : 97,2 sur 150 ko de JS.
- Aucune adresse IP dans le diff.

**Risques résiduels :**
- Deux règles sont des choix à confirmer par Vincent : un `rejected` peut modifier sa candidature tant que le raid est `open`, et un retrait suivi d'une nouvelle candidature repasse par `/e` en `pending`.
- La page n'a pas été vérifiée sur un vrai téléphone.
