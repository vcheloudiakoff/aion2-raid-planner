# Réconciliation forged-idea → ARCHITECTURE-SPINE

Source : `_bmad-output/forge/aion2-raid-sans-compte/forged-idea.md` (verdict HARDENED, 2026-11-05).
Cible : `ARCHITECTURE-SPINE.md` (draft, 2026-11-05). Lens : ce qui n'a pas atterri ou contredit la
source. Ce qui est bien couvert (liens porteurs de droits, SQLite, un conteneur, anti-abus Q1, solveur
déterministe, i18n, purge, « Mes événements », doublons `Pseudo[SRV]`, rejets non réintroduits sauf le
cas 7) n'est pas listé.

## Verdict

La spine respecte tous les verrous techniques du forge, mais deux gestes utilisateur qui portent le
produit « vitrine » — la re-confirmation à délai choisi et le lien Discord réservé aux acceptés — n'ont
pas de règle, et plusieurs exigences discrètes (quatre gestes de contact, duplication, structure 2×5 de la
salle, seuil CP, administration manuelle) sont absentes ou à moitié modélisées.

## Constats

### 1. Re-confirmation « à un délai choisi par l'organisateur » : aucun délai dans le modèle

- **Source** : verrou Vitrine — « les absents s'absorbent par un banc de remplaçants et une
  re-confirmation par lien à un délai choisi par l'organisateur ».
- **Spine** : AD-4 fixe `confirmation ∈ {unasked, asked, confirmed, declined}` et RECONF deux commandes
  (`request_confirmation`, `answer_confirmation`). Aucune échéance (`confirm_by`), aucun état à
  l'expiration du délai, aucune règle sur ce que voit l'organisateur quand le délai passe sans réponse ;
  la tâche horaire d'AD-14 n'en tient pas compte. Le seul « délai » de la spine est `MIN_SUBMIT_DELAY_MS`.
- **Section** : AD-4, AD-14, carte RECONF.
- **Sévérité** : haute.
- **Correction** : ajouter à la demande de confirmation une échéance UTC choisie par l'organisateur
  (absolue ou relative au créneau), un état `expired` calculé côté serveur à partir de l'horloge injectée
  (AD-11), et dire que le dépassement est une information de l'organisateur, jamais une action automatique
  sur le placement.

### 2. Lien Discord du raid « visible des acceptés » : aucune règle de visibilité

- **Source** : verrou Contact — « lien Discord du raid visible des acceptés » ; verrou Vitrine — « la
  confiance et le vocal se font sur le Discord du raid ».
- **Spine** : AD-7 valide le lien (allowlist d'hôtes Discord), AD-12 compose le message côté client, mais
  rien ne dit que la vue publique `/e/<id>`, la liste publique (CAP-4) et la vue `/p/` d'un `pending`,
  `rejected` ou `withdrawn` ne renvoient pas ce champ. En l'état, la liste publique peut devenir une liste
  d'invitations Discord scrapables, ce que le forge rejette explicitement pour les pseudos.
- **Section** : AD-3, AD-4, carte CAP-4 et CAP-2.
- **Sévérité** : haute.
- **Correction** : écrire dans AD-4 (ou AD-3) que le lien Discord est un attribut de l'événement servi
  uniquement sous droit organisateur ou sous droit participant avec `status = accepted`, et jamais dans
  les DTO publics ni dans la liste.

### 3. Les quatre gestes de contact et la vue participant n'ont ni capability ni requête

- **Source** : verrou Contact — `Pseudo[SRV]` à copier (le principal), lien Discord, message prêt à
  coller, « l'inscrit rouvre son lien et lit son statut ». Verrou Rendez-vous — l'identifiant en jeu est
  `Pseudo[SRV]`.
- **Spine** : la carte des capabilities n'a pas d'entrée CONTACT ; `app::queries` ne contient que
  `organizer_view` et `list_public_events`, aucune `participant_view` (statut, placement titulaire ou
  banc, créneau retenu, nom de salle, demande de confirmation en cours). AD-6 décrit les serveurs avec leur
  faction mais pas le tag en jeu (`[SRV]`) sans lequel l'outil ne peut pas composer `Pseudo[SRV]` tel que
  le client l'affiche. Ce que voit un remplaçant (salle, créneau) n'est pas dit.
- **Section** : carte Capability → Architecture, AD-6, AD-4.
- **Sévérité** : moyenne.
- **Correction** : ajouter une ligne CONTACT (les quatre gestes, dont `queries::participant_view`) à la
  carte, et dans AD-6 un champ `tag` par serveur, exactement tel que le client global l'affiche, avec la
  règle que `Pseudo[SRV]` est composé côté serveur ou par un util unique du client, jamais à la main.

### 4. « Dupliquer l'événement » absent

- **Source** : Gardé en mémoire — « Mes événements » et « Dupliquer l'événement » « sont bon marché et
  peuvent entrer en v1 ».
- **Spine** : « Mes événements » est traité (AD-3, localStorage). La duplication n'apparaît nulle part,
  ni en commande, ni en Deferred ; le memlog la suppose pourtant pour justifier `PURGE_AFTER_DAYS = 7`
  (« le temps de dupliquer un événement et de rejouer avec la même équipe »).
- **Section** : carte CAP-1, AD-3, Deferred.
- **Sévérité** : moyenne.
- **Correction** : trancher le mécanisme — préremplissage de l'écran `new` depuis une entrée de « Mes
  événements » (aucune route, aucun droit nouveau, rien à copier côté serveur) — et le dire dans la carte
  ou en Deferred avec le motif.

### 5. Salle : « deux groupes de 5, rôle par slot » et « Faction : All » non modélisés

- **Source** : verrou Rendez-vous — « l'outil génère un nom de salle unique et rend les 10 slots (deux
  groupes de 5, rôle par slot) » ; « une salle que l'organisateur crée à l'heure verrouillée (Faction :
  All) ».
- **Spine** : AD-4 indexe dix `assignments` 0..9 sans structure 2×5 ni rôle par slot ; AD-18 ne fixe que
  le nom. Le rôle est dérivable de la classe par la référence (AD-6), mais la correspondance index → groupe
  de 5 → slot en jeu, et la consigne « Faction : All », ne sont écrites nulle part ; chaque écran pourra
  l'inventer différemment.
- **Section** : AD-4, AD-18, carte ROOM.
- **Sévérité** : moyenne.
- **Correction** : écrire dans AD-4 que l'index 0..4 est la première équipe et 5..9 la seconde, que le
  rôle d'un slot est celui de la classe du titulaire (référence), et dans AD-18 que la vue de verrouillage
  rend la recette de création en jeu (nom, Faction : All, dix slots avec rôle).

### 6. Seuil CP : AD-18 l'utilise, l'agrégat ne le porte pas

- **Source** : verrou Candidature — « CP déclaré en nombre … pré-filtre ; la vérification se fait en
  jeu » ; point faible 2.
- **Spine** : AD-18 compose le nom de salle avec un « seuil CP » ; AD-4 ne donne à l'événement ni
  `min_cp` ni aucun filtre CP ; AD-13 ne l'a pas en contrainte dure. Le pré-filtre promis par le forge
  n'a donc pas d'attribut, et le nom de salle référence une valeur inexistante.
- **Section** : AD-4, AD-18, AD-13.
- **Sévérité** : moyenne.
- **Correction** : ajouter `min_cp` optionnel à l'événement (fixé par l'organisateur, validé contre les
  bornes de la référence), en faire un simple signal dans la grille (pas une contrainte dure du solveur,
  puisque le forge dit « pré-filtre »), et le citer dans `ROOM_NAME_TEMPLATE` comme optionnel.

### 7. Champ « note » libre : une donnée que le forge n'a pas voulue

- **Source** : verrou Candidature — liste fermée des champs (pseudo, serveur, classe, CP, disponibilités,
  préférence) ; Rejeté — « Pseudo Discord par inscrit : liste de cibles scrapable, contredit données
  minimales ».
- **Spine** : AD-7 borne une « note ≤ 280 caractères sans URL », AD-12 dit « notes restent libres », sans
  préciser si elle vit sur l'événement ou sur la candidature. Si elle est par candidature, c'est un texte
  libre où un handle Discord (qui n'est pas une URL) passe, et le rejet est rouvert par la bande.
- **Section** : AD-7, AD-12, AD-4.
- **Sévérité** : moyenne.
- **Correction** : réserver la note à l'événement (description de l'organisateur) et l'écrire dans AD-4 ;
  la candidature n'a aucun champ libre hors le pseudo.

### 8. Administration : mécanisme fixé, pouvoirs absents ; disjoncteur sans coupure manuelle

- **Source** : verrou Anti-abus — « disjoncteur global de création », « lien d'administration secret dans
  `.env` », « Pertes acceptées : un soir de création perdu, un nettoyage manuel ».
- **Spine** : AD-3 fixe `ADMIN_SECRET` et `/api/admin/*` (variable absente = routes absentes), AD-7 les
  seuils `MAX_*_PER_HOUR`. Aucune capability, aucune commande admin (lister, supprimer un événement,
  retirer de la liste, couper la création) ; le disjoncteur n'est qu'automatique, Vincent ne peut pas le
  déclencher lui-même un soir de spam.
- **Section** : carte Capability → Architecture, AD-7, AD-16.
- **Sévérité** : moyenne.
- **Correction** : ajouter une ligne ADMIN à la carte (commandes `admin::delete_event`,
  `admin::unlist_event`, `admin::set_creation_enabled`, toutes par AD-16) et dire que le disjoncteur a
  deux détentes, le seuil automatique et l'interrupteur admin.

### 9. Q8 « lus sans reconstruire l'image » : le seul chemin documenté reconstruit l'image

- **Source** : Renvoyé à la spine — « fichiers de données éditables lus sans reconstruire l'image ;
  Vincent les met à jour après vérification dans le client ».
- **Spine** : AD-6 recharge depuis `/data/reference/` sur changement de mtime, ce qui satisfait le
  critère ; mais « ajouter un serveur = éditer le TOML, commit, push » déclenche AD-10 qui fait `docker
  compose up -d --build` à chaque push, et AD-8 interdit le bind mount qui permettrait d'éditer le volume
  directement. Le rechargement à chaud n'a donc pas de chemin d'usage réel.
- **Section** : AD-6, AD-10, AD-8.
- **Sévérité** : moyenne.
- **Correction** : dans AD-10, rendre l'étape « copie de `reference/` dans le volume » indépendante du
  build (job conditionné sur `paths: reference/**` qui copie sans reconstruire, ou `docker compose cp`),
  et le citer comme le chemin nominal de mise à jour des données de référence.

### 10. Verrouillage : le choix des créneaux retenus et de K n'est pas spécifié

- **Source** : verrou Modèle — « un événement avec 1 à N créneaux produit K groupes (créneau, 10
  titulaires, banc, salle). K = 1 et tri manuel = booking simple » ; signal de succès — « date sortie de
  la grille ».
- **Spine** : AD-4 dit « après verrouillage 0..K groups » et AD-13 reçoit K en entrée, mais K n'est pas un
  attribut de l'événement, et `lock_event` ne dit pas quels créneaux deviennent des groupes en mode manuel
  (sans solveur). Le banc n'a pas d'ordre de promotion.
- **Section** : AD-4, AD-18, carte CAP-3.
- **Sévérité** : basse.
- **Correction** : écrire que `lock_event` prend la liste des créneaux retenus (un groupe par entrée,
  K = sa longueur, doublon de créneau permis), que les placements sont faits ensuite à la main ou par
  `apply_composition`, et que le banc est ordonné (index) pour que « le prochain remplaçant » soit
  sans ambiguïté.

### 11. « Construite en dernier dans la v1 » : la spine ne garantit pas un v1 sans solveur

- **Source** : verrou Composition — « Proposition corrigeable, jamais imposée. Construite en dernier dans
  la v1 ».
- **Spine** : AD-13 fixe l'interface et le déterminisme, Deferred renvoie l'algorithme à l'epic
  composition. Rien ne dit que verrouillage, placement manuel, salle et re-confirmation doivent
  fonctionner sans `compose()` (ce qui est précisément le cas « K = 1 et tri manuel »).
- **Section** : AD-13, Deferred, carte COMP.
- **Sévérité** : basse.
- **Correction** : ajouter une phrase à AD-13 : « `compose()` est optionnel au flux ; tout placement
  manuel passe par la même commande `apply_composition`, qui ne dépend pas du solveur ».

### 12. Nom de salle : ni charset, ni longueur, ni langue

- **Source** : verrou Rendez-vous — liste des salles « avec recherche par nom » ; signal de succès —
  « salle trouvée par son nom ».
- **Spine** : AD-18 compose code Crockford + libellé court de l'instance + seuil CP. Le libellé court est
  bilingue (AD-6) alors que les deux factions et les deux langues cherchent la même salle ; aucune borne
  de longueur ni de charset du champ « nom de salle » du client global.
- **Section** : AD-18, AD-6.
- **Sévérité** : basse.
- **Correction** : fixer que le nom de salle est indépendant de la langue (code en tête, libellé en
  anglais du client global, tiré d'un champ `room_label` de la référence) et borné par une longueur et
  un charset déclarés dans `instances.toml`, à vérifier en jeu.

### 13. Nom d'hôte « dans une zone qui ne pointe pas vers la maison » : pas de règle

- **Source** : verrou Hébergement — « nom d'hôte gratuit pour le pilote dans une zone qui ne pointe pas
  vers la maison » ; Rejeté — « sous-domaine de la zone du NAS : révèle l'adresse du domicile en une
  requête DNS ».
- **Spine** : AD-9 ne dit que « les blocs Caddy d'exemple ne contiennent aucune adresse ni nom d'hôte
  réel » et `PUBLIC_BASE_URL` en configuration. La contrainte sur la zone DNS elle-même n'est pas portée.
- **Section** : AD-9.
- **Sévérité** : basse.
- **Correction** : ajouter à la Rule d'AD-9 : « l'hôte public pointe vers le VPS dans une zone sans lien
  avec le domicile ; aucun enregistrement de la zone du NAS n'est utilisé ».

### 14. En-têtes : le forge les met à l'edge, la spine les déplace dans le binaire

- **Source** : verrou Hébergement — « l'edge déjà en production : … en-têtes à l'edge ».
- **Spine** : AD-9 laisse à l'edge HSTS et `Server`, et donne au binaire CSP, Referrer-Policy, robots,
  cache. Le memlog motive ce choix (no-referrer propre à ce site, testable en `cargo test`) mais la spine
  ne signale pas la déviation par rapport au forge.
- **Section** : AD-9.
- **Sévérité** : basse.
- **Correction** : une phrase dans AD-9 : « déviation du forge (en-têtes à l'edge) : un en-tête lié à
  l'application vit dans le binaire pour ne pas dépendre de la config d'un autre site ».

### 15. « Jamais » de fiabilité ni d'historique : non inscrit en Prevents

- **Source** : verrou Vitrine — « aucune information de fiabilité, de réputation ni d'historique joueur,
  jamais » ; Rejeté — réputation, note, historique, liste noire.
- **Spine** : AD-4 fait de l'événement l'unique agrégat, ce qui l'empêche de fait, mais aucun AD ne le
  dit ; AD-3 étend « Mes événements » aux participants (bienvenu, mais c'est un historique local qui
  mérite d'être nommé comme tel).
- **Section** : AD-4 (Prevents).
- **Sévérité** : basse.
- **Correction** : ajouter à AD-4 Prevents : « une entité joueur transverse aux événements, une table ou
  un index clé par `Pseudo[SRV]` hors d'un événement ».

## Récapitulatif

| # | Constat | Sévérité |
| --- | --- | --- |
| 1 | Re-confirmation sans délai ni échéance | haute |
| 2 | Lien Discord sans règle de visibilité | haute |
| 3 | Gestes de contact, vue participant, tag `[SRV]` absents | moyenne |
| 4 | « Dupliquer l'événement » absent | moyenne |
| 5 | Salle : 2×5, rôle par slot, Faction : All | moyenne |
| 6 | Seuil CP utilisé par AD-18, absent de l'agrégat | moyenne |
| 7 | Champ « note » libre non sourcé | moyenne |
| 8 | Administration sans pouvoirs, disjoncteur sans coupure manuelle | moyenne |
| 9 | Référence : rechargement sans rebuild sans chemin réel | moyenne |
| 10 | Verrouillage : créneaux retenus, K, ordre du banc | basse |
| 11 | v1 sans solveur non garanti | basse |
| 12 | Nom de salle : charset, longueur, langue | basse |
| 13 | Zone DNS hors domicile non portée | basse |
| 14 | Déviation « en-têtes à l'edge » non signalée | basse |
| 15 | « Jamais » d'historique non inscrit en Prevents | basse |
