---
id: SPEC-epic-2-creer-candidater
companions:
  - commandes-et-droits.md
  - ../spec-epic-1-socle/SPEC.md
  - ../spec-epic-1-socle/reference-schema.md
  - ../spec-epic-1-socle/configuration.md
  - ../../planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md
sources:
  - ../../forge/aion2-raid-sans-compte/forged-idea.md
  - ../../../docs/intent-dump.md
  - ../../../docs/domaine-aion2.md
---

> **Contrat canonique.** Ce SPEC et ses `companions:` forment le contrat complet. Les `AD-n` renvoient à
> `ARCHITECTURE-SPINE.md`, qui fait foi. Le socle (epic 1) est supposé livré.

# Epic 2 — Créer et candidater

## Why

Douleur : monter un Sanctuaire à 10 demande de trouver neuf joueurs de plusieurs serveurs et factions,
aujourd'hui par messages épars sur Discord. Cet epic livre le cœur du booking sans compte : un
organisateur publie un raid en moins d'une minute et reçoit ses liens, les joueurs candidatent en moins
de trente secondes depuis leur téléphone, l'organisateur accepte ou refuse. À la fin, l'outil sert déjà
un booking simple : la date se fixe encore sur Discord, la grille et le verrouillage arrivent en epic 3.

## Capabilities

- **CAP-1**
  - **intent:** Un organisateur publie un événement sur un seul écran et repart avec ses trois liens, sans compte (intent CAP-1, AD-3, AD-4, AD-6, AD-11).
  - **success:** test HTTP : `POST /api/events` avec jeton de formulaire valide crée l'événement en `open` et renvoie `CreatedEvent` avec les liens `/o/`, `/j/` et `/e/` ; l'écran Liens créés présente les trois puces dans l'ordre d'EXPERIENCE *Liens et appareils* et garde « Ouvrir mon raid » désactivé tant que le lien organisateur n'est ni copié, ni partagé, ni déclaré gardé ; création mesurée sous 60 s sur un téléphone réel.
- **CAP-2**
  - **intent:** Un joueur qui ouvre le lien public voit l'essentiel du raid sans voir qui s'est inscrit (AD-4 *Données exposées*).
  - **success:** `/e/<id>` affiche instance, région, portée, créneaux en heure locale avec fuseau, note, comptes par classe et par rôle et « manque 1 Soin » calculés côté serveur, jamais de pseudo ni de `discord_url` ; un événement `unlisted` porte `X-Robots-Tag: noindex` et aucune métadonnée `og:` ; un id inconnu donne le message unique « Ce lien ne mène à rien. »
- **CAP-3**
  - **intent:** Un joueur candidate depuis `/e` (à valider) ou `/j` (accepté d'office) avec pseudo, serveur, classe, CP, disponibilités et un ou plusieurs créneaux préférés (verrou Candidature, AD-4, AD-7).
  - **success:** test HTTP : statut initial `pending` par `/e`, `accepted` par `/j`, `origin` stocké ; serveur hors région ou faction hors portée = 400 avec `fields` ; une préférence sur un créneau non coché = 400 ; plusieurs préférences acceptées ; faction et rôle affichés en lecture seule après choix ; le formulaire reprend pseudo, serveur et classe du dernier usage sur l'appareil ; candidature mesurée sous 30 s sur téléphone réel ; la ligne apparaît chez l'organisateur en ≤ 10 s.
- **CAP-4**
  - **intent:** Un candidat rouvre son lien personnel pour lire son statut, modifier sa candidature ou se retirer (intent CAP-5, verrou Contact).
  - **success:** `/p/<secret>` affiche en tête « Ton lien personnel » puis le statut ; modification permise en `open` (409 en `locked`), sans changer le statut ; retrait = `withdrawn` terminal après confirmation destructive ; le lien Discord n'apparaît que si `accepted` ; un événement `cancelled` s'affiche en lecture seule avec « Raid annulé ».
- **CAP-5**
  - **intent:** L'organisateur trie les candidatures et règle son événement depuis son lien secret (AD-4 *Statut*, *Mutabilité*).
  - **success:** `/o/<secret>` onglet Candidatures : liste rafraîchie toutes les 10 s, liseré or 3 s sur une nouvelle ligne, accepter / refuser / ré-accepter selon AD-4, jamais depuis `withdrawn`, `Pseudo[SRV]` en puce à copier, badge « doublon » sur deux lignes au même `Pseudo[SRV]` ; onglet Réglages : note, Discord, visibilité, CP min et bornes toujours modifiables, créneaux modifiables en `open`, `instance`/`region`/`faction_scope` refusés (409) après la première candidature, annulation après confirmation destructive.
- **CAP-6**
  - **intent:** Le navigateur garde les liens de l'utilisateur pour qu'il retrouve ses raids et candidatures, et permet de dupliquer un raid (AD-3, verrou Identité).
  - **success:** l'accueil liste « Sur cet appareil » depuis `links.v1` (instance, date, Organisateur ou Candidat) avec « Oublier » ; un lien invalide retire son entrée locale ; « Dupliquer » préremplit la création sauf les dates ; « Copier mon lien » sur `/o` et `/p` ; stockage vidé = message « copie-le maintenant », page fonctionnelle.
- **CAP-7**
  - **intent:** Toute l'interface livrée par cet epic existe en français et en anglais, au ton d'EXPERIENCE *Voice and Tone* (verrou Q10, AD-12).
  - **success:** chaque texte des écrans de cet epic vient de `fr.json` / `en.json` (test de mêmes clés), chaque code d'erreur introduit a sa traduction (test `list-error-codes`) ; libellés de jeu lus dans la référence, jamais dans les locales.

## Constraints

- Aucun compte, mot de passe, email ni contact de joueur ; pas de pseudo Discord par inscrit (verrous Identité, Candidature).
- Candidature = pseudo, serveur, classe, CP, disponibilités, préférences, `origin` ; aucun autre champ ; faction déduite du serveur, rôle de la classe (AD-4, AD-6).
- `PublicEvent` ne contient jamais la liste nominative ni `discord_url` ; `discord_url` servi seulement aux droits organisateur et participant `accepted` (AD-4).
- Aucune information de fiabilité, de réputation ni d'historique, jamais (verrou Vitrine).
- Toute écriture est une commande transactionnelle sous le droit du lien ; les POST `create` et `candidature` exigent jeton de formulaire et honeypot (AD-7, AD-16).
- Création et candidature tiennent chacune sur un seul écran à 360 px, sans défilement horizontal (AD-19).
- Le secret ne s'affiche que là où il a été demandé : jamais dans un titre, un aperçu de partage ni un message (EXPERIENCE).

## Non-goals

- Pas de grille joueurs × créneaux, de verrouillage, de groupe, de salle ni de placement (epic 3).
- Pas de solveur (epic 4), de re-confirmation, de liste publique sur l'accueil, d'admin ni de purge (epic 5) ; l'accueil n'affiche que « Sur cet appareil » et l'état vide.
- Pas de rôle choisi par le joueur (« Clerc en DPS ») : rôle dérivé de la classe (spine *Deferred*).
- Pas de notification d'aucune sorte.

## Success signal

- Sur l'URL publique, Vincent crée un raid Ludra à trois créneaux depuis son téléphone en moins d'une minute, poste `/j` et `/e` sur Discord, au moins cinq joueurs de deux serveurs candidatent, il accepte et refuse depuis `/o`, chaque joueur lit son statut sur `/p` ; personne ne demande « comment je me connecte ? ».

## Assumptions

- L'onglet Candidatures (accepter, refuser) est livré ici plutôt qu'en epic 3, sinon un candidat `/e` resterait `pending` et l'epic ne serait pas utilisable.
- Plusieurs préférences par candidature (règle UX) : une préférence booléenne par disponibilité cochée ; la « préférence » d'AD-4 et d'AD-13 se lit comme cet ensemble.
- Durée d'un créneau en minutes, défaut de l'instance ; un créneau qui commence dans le passé est refusé.
- Bornes : 14 créneaux par événement, 200 candidatures par événement, pseudo ≤ 16 caractères lettres et chiffres Unicode, CP entier 0 à 999 999.
- L'annulation d'événement (intent CAP-5) est livrée ici dans Réglages.

## Open Questions

- Quels sont la longueur et le charset exacts d'un pseudo Aion 2 ? Ils bornent la validation et la troncature à 136 px.
