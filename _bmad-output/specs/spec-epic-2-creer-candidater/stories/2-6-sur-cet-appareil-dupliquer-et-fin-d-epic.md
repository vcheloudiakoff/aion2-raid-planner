---
title: "Sur cet appareil, dupliquer et fin d'epic"
type: 'feature'
created: '2026-11-07'
status: 'done'
baseline_revision: '1f72db2ba7a38464884aa85ce9e7fca02618155d'
review_loop_iteration: 0
followup_review_recommended: true
context:
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-epic-2-creer-candidater/stories/2-5-organisateur-reglages-et-annulation.md'
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
  - '{project-root}/deploy/README.md'
warnings: [oversized]
deferred: []
---

<intent-contract>

## Intent

**Problem:** l'accueil n'affiche que « Créer un raid ». Ce qui manque :
- un utilisateur ne retrouve pas ses raids ni ses candidatures sans garder ses liens à la main ;
- on ne peut pas dupliquer un raid ;
- `/o` n'offre pas de copier son propre lien ;
- un lien mort reste dans le navigateur.

Rien ne décrit non plus comment vérifier le signal de succès de l'epic 2 en production (SPEC CAP-6, signal de succès).

**Approach:** tout se passe côté web, sans nouvelle route ni modification serveur :
- **Accueil :** une section « Sur cet appareil » construite depuis `links.v1`. Les titres viennent de la lecture publique, qui n'envoie aucun secret.
- **`/o` :** « Dupliquer » et « Copier mon lien » dans Réglages.
- **Liens morts :** l'entrée locale est retirée sur un lien mort.
- **Production :** un §15 dans `deploy/README.md` pour vérifier le signal de succès.

## Boundaries & Constraints

**Always:**
- **Section « Sur cet appareil »** (`pages/Home.tsx`), placée après le bouton « Créer un raid » :
  - Elle est masquée quand `links.v1` ne contient aucune entrée utilisable. Le reste de la page ne change pas.
  - Une ligne « Organisateur » pointe vers `/o/<secret>` pour chaque entrée qui a un `organizer`.
  - Une ligne « Candidat » pointe vers `/p/<secret>` pour chaque participant.
  - Une entrée qui n'a qu'un `join` n'a pas de ligne.
- **Titres et dates :**
  - Les lignes s'affichent tout de suite depuis le stockage, avec le rôle et un squelette à la place du titre. Le premier rendu n'attend aucune API (AD-19).
  - Ensuite, un `GET /api/events/{id}` public (`getPublicEvent`, sans secret) est lancé une seule fois par `eventId` distinct, au montage, sans rafraîchissement.
  - Le titre vient de `eventTitle` : l'instance, puis le jour du premier créneau à venir, sinon celui du dernier créneau. Un raid `cancelled` montre « Raid annulé ».
  - Ne jamais envoyer un secret de `links.v1` depuis l'accueil : AD-3 n'autorise que le secret du chemin courant.
- **Tri :**
  - Une fois toutes les lectures terminées (`allSettled`), les lignes sont triées une seule fois : créneau à venir le plus proche d'abord, puis les raids passés ou annulés, puis ceux dont la lecture a échoué.
  - Ensuite, plus aucune ligne ne bouge.
- **Lecture publique qui répond `event.not_found` :** toute l'entrée de cet événement est oubliée (`forgetEvent`) et sa ligne disparaît.
- **Bouton « Oublier »** sur chaque ligne :
  - Il ouvre une `ConfirmSheet` : « Ce raid disparaîtra de cet appareil. Sans ton lien, tu ne pourras plus y revenir. » Les boutons sont « Oublier » et « Garder ».
  - Une ligne Organisateur appelle `forgetOrganizer(eventId)`, nouvelle fonction qui retire `organizer` et `join` mais garde les participants.
  - Une ligne Candidat appelle `forgetParticipant`.
  - Rien n'est envoyé au serveur.
- **Lien mort** (`links.ts`) :
  - Une nouvelle fonction `forgetSecret(secret)` retire toute case de `links.v1` qui contient ce secret.
  - Seuls `auth.not_found` et `event.not_found` comptent comme un lien mort. Jamais `auth.wrong_right`, `auth.missing`, une erreur réseau, un 429 ni un 5xx.
  - Appels :
    - `/o`, `/p` et `/j` appellent `forgetSecret(secret)` ;
    - `/e` appelle `forgetEvent(id)` sur `event.not_found` ;
    - ces appels partent des branches de lien mort existantes.
  - `GONE` et `DeadLink` passent dans un module partagé, `web/src/components/DeadLink.tsx`, au lieu de vivre dans `EventPage`.
- **« Dupliquer »** (onglet Réglages de `/o`) :
  - C'est un bouton secondaire, visible aussi en lecture seule sur un raid annulé.
  - Il construit un brouillon depuis la vue enregistrée, pas depuis le brouillon non enregistré. Un helper pur et testé, `duplicateDraft(view)`, s'en charge.
  - Le brouillon reprend `instance`, `region`, `factionScope`, `note`, `discordUrl`, `visibility`, `minCp` et `classBounds`.
  - Il garde le même nombre de lignes de créneau, chacune avec sa durée, mais toutes les dates vides. Une durée égale à celle de l'instance est laissée vide.
  - Le brouillon passe par `navigate("/new", { state: { duplicate } })`.
  - `/new` préremplit son formulaire depuis cet état, avec les règles habituelles : instance désactivée ignorée, bornes ramenées dans les limites, portée ajustée.
  - L'état de navigation est effacé une fois lu, avec `replace`, si bien qu'un rechargement donne un formulaire vierge.
- **« Copier mon lien » sur `/o`** (onglet Réglages, en haut) :
  - un bloc « Ton lien d'organisateur » avec une `CopyChip` de `${origin}/o/${secret}`, et l'avertissement `links.organizerWarning` ;
  - le résultat de `saveOrganizerLink` est récupéré ; s'il vaut false, la page affiche `links.noStorage` (« copie-le maintenant ») ;
  - le commentaire d'en-tête d'`OrganizerPage` est mis à jour ;
  - le secret n'apparaît jamais dans `document.title` ;
  - `/p` a déjà sa puce : rien à y ajouter.
- **`deploy/README.md` §15 « Vérification de fin d'epic 2 » :**
  - Il suit le style du §14 : étapes numérotées avec un titre en gras, des vérifications `curl` sur `raid.example.org` et les résultats attendus en commentaire.
  - Étapes :
    1. Créer un raid Ludra à 3 créneaux depuis un téléphone, en moins de 60 s.
    2. Poster `/j` et `/e`, puis vérifier `X-Robots-Tag: noindex` sur `/o/x` et sur un `/e` inconnu.
    3. Faire candidater au moins 5 joueurs de 2 serveurs, chacun en moins de 30 s. Chaque ligne apparaît sur `/o` en 10 s au plus.
    4. Accepter et refuser ; chaque joueur lit son statut sur `/p`, et le lien Discord n'apparaît que pour un joueur accepté.
    5. Vérifier qu'aucun pseudo n'apparaît dans la lecture publique : `curl …/api/events/<ID> | grep -c '"pseudo"'` doit répondre 0.
    6. Vérifier « Sur cet appareil », « Oublier », « Dupliquer », « Copier mon lien », la navigation privée et un lien mort simulé par les outils de développement.
    7. Constater que personne ne demande comment se connecter.
  - Préalables :
    - le §14 de l'epic 1 a été fait ;
    - les commits de l'epic sont poussés, avec le feu vert de Vincent ;
    - `0002_events.sql` a été relue, puisqu'elle devient immuable une fois poussée.
  - Aucune adresse IP, aucun nom d'hôte réel ni aucun secret n'apparaît dans le texte.
- **Locales :** tous les textes en fr et en en, ton *Voice and Tone*.

**Never:**
- Ajouter une route serveur, une migration ou un endpoint groupé.
- Stocker un résumé dans `links.v1`, dont la forme est fixée par AD-3.
- Rafraîchir l'accueil.
- Envoyer un secret stocké ailleurs que sur sa propre page.
- Retirer une entrée locale sur une erreur réseau ou sur `auth.wrong_right`.
- Ajouter une liste publique (epic 5).
- Pousser, ou exécuter la vérification de production : ce sont les gestes de Vincent.
- Écrire `SPEC.md`, la spine ou `sprint-status.yaml`.

## I/O & Edge-Case Matrix

| Scénario | Entrée / état | Résultat attendu | Gestion d'erreur |
|----------|---------------|------------------|------------------|
| Accueil vide | `links.v1` absent ou vide | section masquée ; le reste de l'accueil est inchangé | — |
| Accueil rempli | 1 organisateur, 2 candidatures, 1 entrée `join` seule | 3 lignes, titres remplis après lecture publique, triées une fois | lecture en échec → ligne gardée, sans titre, en fin de liste |
| Événement disparu | lecture publique → `event.not_found` | entrée de l'événement oubliée, ligne retirée | — |
| Oublier | « Oublier » puis confirmation | entrée retirée de `links.v1`, ligne retirée ; « Garder » ne change rien | — |
| Lien mort sur `/o`, `/p` ou `/j` | `auth.not_found` | message de lien mort et `forgetSecret` | `auth.wrong_right` ou réseau → rien n'est retiré |
| Dupliquer | Réglages → « Dupliquer » | `/new` prérempli sauf les dates, même nombre de créneaux ; un rechargement donne un formulaire vierge | instance désactivée → instance par défaut |
| Copier mon lien `/o` | Réglages | puce avec l'URL `/o` complète ; avertissement de stockage si l'écriture a échoué | — |

</intent-contract>

## Code Map

- `web/src/api/links.ts:15-118` -- `EventLinks`, `sanitizeEvent`, `readLinks`, `eventLinks`, `update`, les fonctions `save*`, `forgetParticipant` et `forgetEvent`. À ajouter : `forgetOrganizer` et `forgetSecret`.
- `web/src/pages/Home.tsx` -- 21 lignes, sans test propre. `App.test.tsx:46-54` vérifie l'état vide et l'absence de « Sur cet appareil ».
- `web/src/api/candidatures.ts:17` -- `getPublicEvent(eventId)`, lecture sans secret.
- `web/src/components/EventSummary.tsx:27-32` -- `eventTitle(event, reference, lang, now)`.
- `web/src/pages/EventPage.tsx:21-24,77-80,159-174` -- `REFRESH_MS`, `GONE`, branche de lien mort et `DeadLink` : à déplacer dans `components/DeadLink.tsx`.
- `web/src/pages/OrganizerPage.tsx:8-9,138-141,174-177,225-230,281` -- commentaire d'en-tête, `saveOrganizerLink` (résultat ignoré) et branches de lien mort.
- `web/src/pages/ParticipantPage.tsx:80,114,171-174,278-286` -- prop `origin`, `url`, branche de lien mort, `noStorage` et puce.
- `web/src/components/EventSettings.tsx:77-96,582-625` -- `ReadOnly` (raid annulé), section d'annulation et modèle `ConfirmSheet`.
- `web/src/pages/NewEvent.tsx:92-139` -- état `Form` et ses replis. Aucun préremplissage pour l'instant.
- `web/src/api/types.ts:212` -- `OrganizerEvent`, qui contient tout ce que la duplication doit copier.
- `web/src/test/fixtures.ts` -- `ORGANIZER`, `JOIN`, `EVENT_ID`, `PARTICIPANT`, `CANDIDATURE_ID`, `publicEvent`, `participantView` et `organizerEvent`.
- `deploy/README.md:287-341` -- §14, modèle du §15.

## Tasks & Acceptance

**Execution:**
- `web/src/api/links.ts` -- `forgetOrganizer` et `forgetSecret`, avec leurs tests.
- `web/src/components/DeadLink.tsx` -- `GONE`, `DeadLink` et un helper `isDeadLinkCode` (`auth.not_found` ou `event.not_found`), extraits d'`EventPage`.
- `web/src/pages/{EventPage,OrganizerPage,ParticipantPage}.tsx` -- oubli local sur lien mort ; « Copier mon lien » et l'avertissement de stockage sur `/o`.
- `web/src/pages/Home.tsx` et `web/src/components/OnThisDevice.tsx` -- section, lectures publiques, tri unique, « Oublier » avec confirmation.
- `web/src/lib/duplicate.ts`, `components/EventSettings.tsx` et `pages/NewEvent.tsx` -- `duplicateDraft`, bouton « Dupliquer », préremplissage de `/new`.
- `web/src/locales/fr.json` et `en.json` -- textes.
- `deploy/README.md` -- §15.
- Tests web, dans `web/src/**/*.test.tsx` : chaque ligne de la matrice, plus les cas suivants :
  - aucun en-tête `X-Link-Secret` envoyé depuis l'accueil ;
  - secret absent du titre ;
  - `NewEvent` sans état garde son comportement ;
  - routage dans `App`.

**Acceptance Criteria:**
- Étant donné un raid créé puis une candidature faite sur le même navigateur, quand on ouvre l'accueil, alors on voit deux lignes, « Organisateur » et « Candidat », avec l'instance et la date, et aucune requête de l'accueil ne porte de secret.
- Étant donné le §15 de `deploy/README.md`, quand Vincent le suit après le push, alors chaque étape du signal de succès du SPEC a une vérification et un résultat attendu.

## Spec Change Log

## Review Triage Log

### 2026-11-07 — Review pass
- verdicts: 31 findings — high 0, medium 8, low 21, false 2, maybe-false 0
- findings:
  - `[low]` `[reject]` La vérification de production est une procédure, pas une exécution — la spec en fait un geste de Vincent après son push (`done_checkpoint`).
  - `[low]` `[patch]` Le §15 ne vérifie pas « deux serveurs » — l'étape 3 vérifie désormais au moins deux tags `[SRV]` différents sur `/o`.
  - `[false]` `[reject]` « Dupliquer » depuis `/o` plutôt que depuis l'accueil — choix noté dans les Design Notes : la vue publique n'a ni `discordUrl` ni `classBounds`.
  - `[low]` `[reject]` Tests en happy-dom avec réseau simulé — les corps simulés correspondent aux codes du serveur, ce que l'audit a vérifié.
  - `[low]` `[reject]` Le critère d'acceptation remplit le stockage directement — avec les fonctions mêmes qu'utilisent la création et la candidature.
  - `[medium]` `[patch]` Un `/o` mort laisse `join` stocké pour toujours — `forgetSecret` retire aussi le `join` de l'événement d'un lien organisateur ; testé.
  - `[medium]` `[patch]` Lien mort découvert par une commande participant ou par l'envoi d'une candidature, et non oublié — même traitement qu'au chargement ; testé sur les quatre chemins.
  - `[low]` `[reject]` Libellé « Copier mon lien » non littéral, puce dans Réglages — même intitulé que Liens créés et `/p` ; Réglages évite d'afficher le secret sur l'onglet par défaut.
  - `[low]` `[reject]` Avertissement de stockage sur `/o` seulement si l'écriture échoue — c'est le seul cas où le lien est réellement perdu.
  - `[low]` `[patch]` Tri non testé pour un raid passé non annulé et pour un créneau passé suivi d'un créneau à venir — test ajouté.
  - `[low]` `[patch]` `nextKey` après une duplication non testé — test ajouté.
  - `[medium]` `[patch]` « Oublier » ignore l'échec d'écriture — la ligne reste et la feuille affiche l'erreur ; testé.
  - `[low]` `[patch]` Focus perdu quand une lecture retire la dernière ligne pendant que sa feuille est ouverte — repli sur le `h1` ; testé.
  - `[medium]` `[patch]` `join` orphelin (Edge Case Hunter) — même correctif que `forgetSecret`.
  - `[false]` `[reject]` Lectures parallèles sans limite de concurrence — il faudrait plus de 600 événements stockés pour toucher le quota `read`.
  - `[low]` `[patch]` Un créneau en cours est classé comme passé — le tri compte jusqu'à la fin du créneau.
  - `[low]` `[patch]` « Dupliquer » reste actif pendant une commande — désactivé pendant `busy`.
  - `[low]` `[reject]` Durée hors bornes dans un état de navigation forgé — l'état vient de notre propre helper ; le serveur reste la garde.
  - `[low]` `[patch]` La simulation de lien mort du §15 ne marche pas telle qu'écrite — l'étape dit de changer un caractère base64url, puis de recharger l'accueil.
  - `[medium]` `[patch]` `join` laissé par un `/o` mort (Blind Hunter) — même correctif que `forgetSecret`.
  - `[low]` `[patch]` Simulation du §15 (Blind Hunter) — même correctif.
  - `[low]` `[patch]` « Dupliquer » jette la saisie non enregistrée — l'indication précise que seuls les réglages enregistrés sont copiés, et le bouton est désactivé pendant une commande.
  - `[medium]` `[patch]` Liens morts sur commande non oubliés (Blind Hunter) — même correctif.
  - `[medium]` `[patch]` Échec d'écriture d'« Oublier » ignoré (Blind Hunter) — même correctif.
  - `[low]` `[patch]` Texte de confirmation faux pour une ligne organisateur quand une ligne candidat du même raid reste — quatre messages selon le rôle et selon les lignes qui restent.
  - `[medium]` `[patch]` CP minimum et bornes dupliqués cachés dans des « Options » repliées — « Options » s'ouvre quand le brouillon les porte ; testé.
  - `[low]` `[patch]` L'avertissement de stockage de `/o` n'a rien à copier sur l'onglet Candidatures — `organizer.noStorage` renvoie à l'onglet Réglages.
  - `[low]` `[patch]` Les vérifications `curl` du §15 sont trop faibles — `/j/x` et `/p/x` vérifiés, recherche d'un vrai pseudo, recherche `discord.gg` retirée.
  - `[low]` `[patch]` Le §15 ne force pas le cas « stockage vidé » — blocage des données du site dans les réglages du navigateur, avertissements `/p` et `/o`, et « Copier mon lien » sur `/p`.
  - `[low]` `[patch]` Les raids passés sont listés du plus ancien au plus récent — triés par dernier créneau, le plus récent d'abord.
  - `[low]` `[patch]` Focus perdu (Blind Hunter) — même correctif que le repli du focus.

## Design Notes

**Titres par la lecture publique :** AD-3 fige la forme de `links.v1` et interdit d'envoyer un secret stocké hors de sa page. La lecture publique `GET /api/events/{id}` ne demande aucun secret et donne l'instance et les créneaux. Une dizaine de lectures au chargement reste très loin du quota `read` (600 par minute).

**Dupliquer depuis `/o`, pas depuis l'accueil :** la vue publique n'a ni `discordUrl` ni `classBounds`. La vue organisateur est lue avec le secret du chemin courant, ce que permet AD-3. On s'écarte de la lettre d'AD-3 (« depuis cette entrée »), mais ni nouvelle route ni nouveau droit ne sont nécessaires, conformément à l'esprit.

**« Oublier » confirmé :** la perte est définitive, puisqu'aucune récupération n'existe sans compte. EXPERIENCE ne liste pas ce geste parmi les confirmations destructives, d'où ce choix `[ASSUMPTION]`.

## Verification

**Commands:**
- `cd web && npm run typecheck && npm test && npx vite build && node scripts/check-budget.mjs` -- expected: vert, JS sous 150 KB.
- `cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && REQUIRE_WEB_DIST=1 cargo test --locked` -- expected: tout passe (aucun changement serveur attendu).
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: OK.
- `sh deploy/check-hardening.sh && sh deploy/tests/check-hardening.test.sh && sh deploy/tests/plan-deploy.test.sh` -- expected: OK.
- `grep -nE "([0-9]{1,3}\.){3}[0-9]{1,3}" deploy/README.md` -- expected: seulement des adresses de documentation (127.0.0.1, 192.0.2.x).

## Auto Run Result

Status: done

**Résumé :** la story se fait côté web seulement, sans aucun changement serveur.

- **Accueil :** la section « Sur cet appareil » liste les entrées de `links.v1` :
  - une ligne « Organisateur » et une ligne « Candidat » par entrée ;
  - les titres viennent de la lecture publique, faite sans secret ;
  - les lignes sont triées une seule fois (en cours ou à venir, puis passées ou annulées, puis lectures en échec) ;
  - « Oublier » est confirmé, avec un message selon le rôle.
- **Liens morts :** sur `auth.not_found` ou `event.not_found`, l'entrée locale est oubliée, que l'erreur arrive au chargement, au rafraîchissement ou sur une commande, sur `/o`, `/p`, `/j` et `/e`. Un `/o` mort retire aussi le `join`. `GONE` et `DeadLink` sont partagés dans `components/DeadLink.tsx`.
- **`/o`, onglet Réglages :**
  - « Ton lien d'organisateur » à copier, avec l'avertissement de stockage ;
  - « Dupliquer », qui copie la vue enregistrée sans les dates et préremplit `/new` ; « Options » s'ouvre si le brouillon porte un CP minimum ou des bornes.
- **`deploy/README.md` §15 :** la vérification de fin d'epic 2, en sept étapes, à faire par Vincent.

**Fichiers :**
- `web/src/api/links.ts` : `forgetOrganizer` et `forgetSecret`.
- `web/src/components/{DeadLink,OnThisDevice,EventSettings}.tsx`.
- `web/src/lib/duplicate.ts`.
- `web/src/pages/{Home,EventPage,ParticipantPage,OrganizerPage,NewEvent}.tsx`, `App.tsx` et les locales.
- `web/src/test/dom.ts` : `spyStorageWrites`.
- `deploy/README.md`.

**Relecture :** 31 remarques, dont 22 corrigées en 12 correctifs et 9 écartées (2 fausses), chacune avec sa raison dans le journal. Rien n'est reporté.

**Nouvelle relecture recommandée :** oui. Quatre entrées de gravité moyenne ont été corrigées. Le risque non vérifié est que l'oubli sur lien mort, appliqué aux commandes participant et aux envois de candidature, efface un lien encore valide si un code `not_found` arrivait pour une autre raison.

**Vérification :**
- fmt et clippy `-D warnings` passent, et `REQUIRE_WEB_DIST=1 cargo test` aussi : 282 tests, 0 échec.
- `list-error-codes` : 41 codes, présents dans les 2 locales.
- `validate-reference`, le contrôle des crates réseau, le durcissement (86 tests) et `plan-deploy` (20 tests) passent.
- Web : tsc et vitest passent (304 tests). Le budget est respecté : 106,8 sur 150 ko de JS.
- Aucune adresse IP ni aucun nom d'hôte réel dans le diff ni dans le §15.

**Risques résiduels :**
- Le signal de succès de l'epic 2 n'est pas vérifié. Il faut d'abord le push, avec le feu vert de Vincent, puis le §15 sur un vrai téléphone, avec de vrais joueurs.
- La confirmation d'« Oublier » est un `[ASSUMPTION]`.
