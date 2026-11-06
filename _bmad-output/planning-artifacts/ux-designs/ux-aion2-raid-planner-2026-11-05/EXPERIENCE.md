---
name: aion2-raid-planner
status: final
created: '2026-11-05'
updated: '2026-11-06'
sources:
  - _bmad-output/forge/aion2-raid-sans-compte/forged-idea.md
  - _bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - docs/intent-dump.md
  - docs/domaine-aion2.md
---

# aion2-raid-planner — spine d'expérience

Rédigée en fast path le 2026-11-05, relue et acceptée par Vincent le 2026-11-06. Les verrous du forge
et les `AD` de la spine d'architecture sont repris par référence, sans être rediscutés. Sur tout conflit
avec une maquette, cette spine et `DESIGN.md` font foi.

## Foundation

- **Surface unique** : web mobile d'abord, à une largeur de référence de 360 px, sans aucun défilement
  horizontal (AD-19). Sur PC (alt-tab en jeu), c'est la même interface centrée dans une colonne de
  `{spacing.content-max}`. Pas d'application native, pas de PWA en v1.
- **Pile UI** : React + Tailwind utilitaire, sans bibliothèque de composants (AD-19 limite les
  dépendances d'exécution). Chaque comportement décrit ici se code à la main : feuille du bas, toast,
  copie.
- **Identité visuelle** : `DESIGN.md`. Sombre par défaut, thème clair en réglage.
- **Langues** : français et anglais dès la v1 (AD-12). Tous les textes vivent dans `fr.json` /
  `en.json`, et le choix est mémorisé localement.
- **Aucun compte** : les droits passent par les liens (AD-3). Toute l'UX de « qui je suis » est une UX
  de garde de liens (voir *Liens et appareils*).

## Information Architecture

| Surface | Route | Atteinte depuis | Droit | Rôle |
|---|---|---|---|---|
| Accueil | `/` | ouverture directe, logo | public | liste publique filtrable (CAP-4), « Sur cet appareil », bouton Créer |
| Créer un raid | `/new` | Accueil, « Dupliquer » | public | formulaire sur un seul écran (CAP-1) |
| Liens créés | `/new` (étape 2) | soumission de la création | organisateur | les trois liens, affichés une fois, à copier |
| Page de l'événement | `/e/<id>` | liste, Discord | public | résumé sans liste nominative, formulaire de candidature (CAP-2) |
| Candidature directe | `/j/<secret>` | lien direct posté par l'organisateur | direct | même formulaire, accepté d'office |
| Ma candidature | `/p/<secret>` | après candidature, « Sur cet appareil » | participant | statut, placement, salle, confirmation, modifier, se retirer (CAP-5, CONTACT) |
| Organisateur : Candidatures | `/o/<secret>` (onglet par défaut) | Liens créés, « Sur cet appareil » | organisateur | trier, accepter, refuser, copier `Pseudo[SRV]` |
| Organisateur : Disponibilités | `/o/<secret>` onglet | onglets | organisateur | grille joueurs × créneaux, couverture par classe, Verrouiller (CAP-3) |
| Organisateur : Composition | `/o/<secret>` onglet, après verrouillage | onglets | organisateur | proposition du solveur ou placement à la main (COMP), chargé à la demande |
| Organisateur : Salle | `/o/<secret>` onglet, après verrouillage | onglets | organisateur | recette en jeu par groupe, message Discord, re-confirmation (ROOM, RECONF) |
| Organisateur : Réglages | `/o/<secret>` onglet | onglets | organisateur | note, lien Discord, visibilité, CP min, bornes, créneaux (en `open`), annuler, dupliquer |
| Administration | `/a/<secret>` | lien gardé par Vincent | admin | liste, retirer de la liste, supprimer, couper la création écran minimal |
| Lien invalide | toute route secrète | lien faux, expiré, purgé | aucun | même message pour inconnu et expiré |

L'écran organisateur affiche une **barre d'onglets fixe en haut**, sous le titre, et chaque onglet
déroule verticalement. Les onglets Composition et Salle n'apparaissent qu'en `locked`. Les feuilles du bas
ne s'empilent jamais sur plus d'un niveau.

L'en-tête global contient le nom de l'outil à gauche et, à droite, un bouton qui ouvre une feuille
« Langue · Thème ». Il n'y a pas de menu hamburger.

## Voice and Tone

Le ton est celui d'un officier de légion qui écrit sur Discord : bref et direct, avec le tutoiement en
français et un registre familier en anglais. Il ne s'excuse pas, ne met pas de point
d'exclamation et n'emploie pas de jargon technique.

| Faire | Ne pas faire |
|---|---|
| « Garde ce lien : c'est le seul moyen de gérer ton raid. » | « Veuillez conserver précieusement votre URL d'administration » |
| « Copié » | « ✓ Copié dans le presse-papiers avec succès ! » |
| « 12 dispo · manque 1 Soin » | « 12 joueurs sont disponibles pour ce créneau » |
| « Ce lien ne mène à rien. Il a peut-être expiré. » | « Erreur 404 : ressource introuvable » |
| « Trop de créations depuis ta connexion. Réessaie dans une heure. » | « Rate limit exceeded » |
| « Proposition : 2 groupes complets. Rien n'est enregistré tant que tu n'appliques pas. » | « Le solveur a terminé » |

Règles :

- Les heures sont toujours affichées en heure locale, avec le jour court (« mar. 21:00 »), et le fuseau
  est indiqué une fois en tête d'écran.
- `Pseudo[SRV]` s'écrit toujours sous cette forme exacte.
- Les noms de classes, de serveurs et d'instances viennent de la référence (libellé `fr`, sinon `en`),
  jamais des locales.

## Component Patterns

Comportemental. Les spécifications visuelles sont dans `DESIGN.md.Components`.

| Composant | Usage | Règles de comportement |
|---|---|---|
| Puce à copier | `Pseudo[SRV]`, nom de salle, liens | Un toucher copie, puis toast « Copié » 1,5 s. Si l'API presse-papiers échoue, le texte est sélectionné pour une copie manuelle. Aucun effet de bord serveur. |
| Lien à garder | écran Liens créés, Ma candidature | Puce à copier + bouton « Partager » (Web Share) quand il est disponible. Le lien organisateur n'est jamais tronqué au milieu. |
| Puce de créneau | grille (vue par créneau), filtres | Sélection unique, avec un retour à la ligne quand les puces débordent, jamais un défilement horizontal. Ordre chronologique. |
| Cellule de disponibilité | formulaire de candidature, matrice | Bascule ✓ / –, plusieurs créneaux cochables. Le bouton ☆ (ou un appui long) marque un créneau préféré ; **plusieurs préférences permises**, chacune parmi les créneaux cochés. |
| Ligne de candidature | Candidatures, grille, réserve | Un toucher ouvre la feuille de la candidature (détails, accepter / refuser, copier). Les actions ne se déclenchent jamais par balayage. |
| Couverture par classe | Disponibilités, Composition | Une ligne par classe bornée : « Clerc 1 / min 2 ». Sous le minimum, l'état passe en texte `danger`. Au-dessus du maximum : « 3 / max 2 ». |
| Feuille du bas | réserve, détail de candidature, verrouillage, langue et thème | Le focus entre dans la feuille et revient au déclencheur à la fermeture. Fermeture par Échap, par glissement ou par le bouton Fermer. |
| Bouton principal | un par écran | Désactivé seulement si le formulaire est invalide, et la raison s'affiche sous le bouton. Pendant l'envoi, libellé « Envoi… », pas de spinner seul. |
| Confirmation destructive | annuler, se retirer, refuser, supprimer un créneau avec groupes | Une feuille qui nomme la conséquence (« 2 groupes de ce créneau seront supprimés »). Le bouton reprend le verbe : « Supprimer le créneau ». |
| Stepper | bornes par classe, nombre de groupes par créneau | Boutons − / + de 44 px et valeur au centre, saisissable. |

## State Patterns

| État | Surface | Traitement |
|---|---|---|
| Liste publique vide | Accueil | « Aucun raid ouvert pour l'instant. » + « Créer un raid ». Pas de faux contenu. |
| Aucun lien sur cet appareil | Accueil | La section « Sur cet appareil » est masquée. |
| Stockage local indisponible ou vidé | toutes | La page marche. Sur Liens créés et Ma candidature : « Ce navigateur ne gardera pas le lien : copie-le maintenant. » |
| Lien invalide ou expiré | `/o`, `/p`, `/j`, `/e` | Un seul message pour tous ces cas, puis « Retour à l'accueil ». L'entrée locale correspondante est retirée de « Sur cet appareil ». |
| Premier rendu | toutes | Rendu immédiat depuis la référence en cache (AD-19) ; squelettes de lignes pour les données de l'événement. Pas de plein écran de chargement. |
| Rafraîchissement (toutes les 10 s) | `/o`, `/p`, `/e` | Mise à jour silencieuse. Une nouvelle candidature apparaît avec un liseré or pendant 3 s. La région live n'annonce que les changements de statut du participant. |
| Hors ligne ou serveur injoignable | toutes | Bandeau discret « Hors ligne · dernières données de 21:04 ». Les actions sont désactivées avec la raison. |
| Nouvelle version déployée | toutes | Bandeau « Nouvelle version · Recharger ». Pas de rechargement forcé : un formulaire en cours n'est jamais perdu. |
| Création coupée (disjoncteur ou admin) | `/new` | « La création de raids est en pause. Les raids existants marchent toujours. » Le formulaire reste lisible. |
| Trop de requêtes | formulaires | Message de `errors.<code>` avec une durée approximative. La saisie est conservée. |
| Jeton de formulaire expiré | formulaires | Nouveau jeton et nouvel essai automatique, une fois, sans rien dire. Si ça échoue encore, message d'erreur. |
| Brouillon de composition et données changées | Composition | Bandeau « 2 candidatures ont changé depuis ton brouillon » + « Voir » ; le brouillon n'est jamais écrasé. |
| Doublon `Pseudo[SRV]` | Candidatures, grille | Badge « doublon » sur les deux lignes. L'organisateur choisit laquelle refuser. |
| Événement déverrouillé | `/p` | Le placement et la salle disparaissent de la vue participant. Message : « Le raid est revenu en préparation. » |
| Événement annulé | `/e`, `/p` | Bandeau « Raid annulé ». Lecture seule jusqu'à la purge. |
| Confirmation demandée | `/p` | Carte en tête de page : « Tu confirmes pour mar. 21:00 ? » Oui / Non, échéance en heure locale. |
| Échéance dépassée | `/p`, Salle | Côté participant : « Trop tard pour répondre. Préviens l'organisateur sur Discord. » Côté organisateur : badge « expirée ». Aucun effet sur le placement. |

## Interaction Primitives

- Toucher pour agir. L'appui long ne sert qu'à marquer la préférence, qui a aussi un bouton visible.
- Placement en deux temps : toucher une cible (slot libre, banc) ouvre la réserve des candidatures
  éligibles ; toucher une candidature la place. Un slot occupé propose « Retirer », « Au banc » et
  « Échanger » (puis toucher un autre slot).
- Glisser vers le bas pour fermer une feuille. Pas d'autre geste de balayage.
- La copie est le geste principal : tout ce qui se recopie dans le jeu ou dans Discord est une puce à
  copier.
- **Interdits** : glisser-déposer, défilement horizontal, carrousels, notifications, sons, animations
  d'entrée, mises à jour automatiques qui déplacent la ligne sous le doigt.

## Accessibility Floor

Comportemental. Le contraste visuel est dans `DESIGN.md`.

- Visé : WCAG 2.2 AA, vérifié à la main sur les écrans clés, sans audit
- La matrice est un vrai `<table>` : en-têtes `th scope` pour les joueurs et les créneaux, et chaque
  cellule annonce « Pseudo, mar. 21:00, disponible ».
- Les bascules utilisent `aria-pressed`, les onglets le motif `tablist`, et les feuilles `role="dialog"`
  avec `aria-modal` et un focus piégé.
- Aucun état n'est porté par la couleur seule : la disponibilité a ✓ / –, les statuts un libellé, la
  préférence ☆.
- Chaque cible fait au moins 44 × 44 px. Le focus clavier reste visible dans les deux thèmes.
- `prefers-reduced-motion` coupe le liseré animé et les transitions des feuilles.
- À 200 % de zoom, la matrice bascule en vue par créneau : la règle porte sur la largeur disponible,
  pas sur le nombre de créneaux seul.
- L'attribut `lang` de la page suit la langue choisie.

## Grille joueurs × créneaux

C'est le premier des deux écrans à risque du forge.

**Choix de la vue** : matrice si le nombre de créneaux est ≤ 4 **et** que la largeur utile est ≥ 328 px.
Sinon, vue par créneau. Le sélecteur « Matrice · Par créneau » est visible quand les deux vues sont
possibles, et la matrice est alors la vue par défaut. Quand la matrice est impossible, le sélecteur la montre
grisée avec sa raison (« ≤ 4 créneaux »).
→ Référence visuelle : [`mockups/key-grille.html`](mockups/key-grille.html). La spine fait foi en cas de conflit.

**Matrice.**

- La colonne joueur fait 136 px : `Pseudo` sur une ligne (tronqué avec une ellipse et le nom complet dans
  la feuille), puis classe et CP en `meta` sur la ligne suivante.
- Il y a ensuite une colonne de 44 px par créneau. L'en-tête de colonne porte le jour court et l'heure sur
  deux lignes, et une ligne de pied donne le compte des disponibles parmi les `accepted`.
- Le tri par défaut place les `accepted` d'abord, puis les candidatures par nombre de créneaux cochés.
  Un filtre en puces : Tous / Acceptés / En attente.
- Toucher un en-tête de créneau le sélectionne et affiche sous la grille la couverture par classe pour ce
  créneau.

**Vue par créneau.**

- Un bandeau de puces de créneau s'étale sur plusieurs lignes au besoin, chaque puce portant le compte
  des disponibles.
- Sous le bandeau viennent la couverture par classe du créneau sélectionné, puis la liste des
  candidatures disponibles.
- Enfin, un repli « Pas dispo (5) » est fermé par défaut.

**Commun aux deux vues.**

- Le bouton principal « Verrouiller » ouvre une feuille avec une ligne par créneau :
  - une case pour retenir le créneau ;
  - un stepper du nombre de groupes, à 1 par défaut, borné par les disponibles divisés par
    `group_size` ;
  - l'indication « assez pour 2 groupes ».
- La feuille se valide d'un seul geste par « Verrouiller 2 groupes ». Verrouiller la date tient donc en
  deux touchers, ce qui répond à CAP-3.
- En `locked`, la grille est en lecture seule et « Déverrouiller » apparaît dans Réglages (créneau à venir
  seulement, AD-4).

## Écran de composition

C'est le second écran à risque. Il est chargé à la demande (AD-19).

**En-tête.**

- Une puce par groupe (« G1 · mar. 21:00 »), en sélection unique, sur plusieurs lignes au besoin.
- Sous les puces, l'état du groupe : « 8 / 10 · manque 1 Soin ».

**Corps : le groupe sélectionné.**

- Équipe 1 : cinq slots de salle verticaux. Équipe 2 : cinq slots. Puis le banc, ordonné, avec un
  bouton « + Ajouter au banc ».
- Chaque slot affiche son numéro, le rôle de son titulaire et `Pseudo[SRV]`, ou « libre ».

**Barre d'action collée en bas.**

- Hors brouillon : « Composer automatiquement », avec une feuille qui demande l'objectif (« Équilibrer
  les CP entre groupes » par défaut, ou « Le plus fort d'abord ») et précise « rien n'est enregistré
  avant Appliquer ».
- Pendant le calcul (budget d'une seconde, AD-13) : « Calcul… ».
- La proposition remplit tous les groupes en **brouillon**. Les slots changés portent le badge
  « Proposé », et la barre devient « Appliquer » (principal) + « Abandonner ».

**Placement à la main.**

- Toute modification manuelle ouvre aussi un brouillon local, avec la même barre « Enregistrer » /
  « Abandonner ».
- `apply_placement` reçoit le placement complet (AD-4). Un seul enregistrement couvre donc tous les
  groupes.
- Le serveur ne reçoit rien avant ce geste.

**Réserve (feuille du bas).**

- Elle est ouverte par le toucher d'une cible.
- Elle liste les candidatures `accepted` éligibles dans cet ordre : disponibles au créneau et non
  placées, triées par classe manquante puis par CP.
- Viennent ensuite « Placées dans un autre groupe » (le toucher déplace la candidature).
- Enfin, repliées par défaut, « Pas dispo à ce créneau ».

**Contraintes à la main.** Placer une candidature non disponible au créneau, ou dépasser une borne de
classe, est permis avec un avertissement en ligne : l'organisateur connaît ses joueurs mieux que l'outil
(accepté par Vincent). Cette règle doit être confirmée contre la validation de `apply_placement` au moment de la
spec. L'unicité (une candidature dans un seul slot) reste dure, et l'interface la garantit en déplaçant
plutôt qu'en dupliquant.

## Salle et message Discord

Il y a une carte par groupe, dans l'ordre des créneaux.

**Le nom de la salle.** Il s'affiche en `display` dans une puce à copier, et c'est le geste de
l'organisateur au moment de créer la salle en jeu.

**La recette en trois lignes** (AD-18) :

1. « Crée une salle de Sanctuaire » + le libellé de l'instance.
2. « Nom : » + le nom de la salle.
3. « Faction : All », si `faction_scope = all`.

**La salle.** Équipe 1 puis Équipe 2, slot par slot : numéro, rôle, et `Pseudo[SRV]` en puce à copier,
pour retrouver chaque joueur dans le jeu à la demande de rejoint. Le banc vient ensuite.

**Les actions.**

- « Copier le message Discord » (principal).
- « Demander confirmation » : une feuille pour choisir les candidatures (titulaires cochés par défaut)
  et l'échéance (raccourcis « 2 h avant », « la veille », ou une heure libre).
- « Régénérer le nom » est rangé dans un menu « … », avec une confirmation qui prévient que le nom posté
  sur Discord deviendra faux.

**Le message Discord.** C'est un gabarit dans les locales.

- Il contient l'instance, l'heure au format d'horodatage Discord `<t:UNIX:F>`, que chaque lecteur voit
  dans son propre fuseau
- Il contient le nom de la salle, les titulaires par équipe en `Pseudo[SRV]`, le banc et le lien public
  `/e/` de l'événement.
- Il ne contient jamais de lien secret.

## Liens et appareils

- **Création** : l'écran Liens créés présente trois puces, dans cet ordre :
  - « Ton lien d'organisateur », secret, avec l'avertissement ;
  - « Lien direct », à donner à ceux que tu acceptes d'office ;
  - « Lien public », à poster partout.

  « Ouvrir mon raid » reste désactivé tant que le lien organisateur n'a pas été copié ou partagé, ou que
  la case « J'ai gardé mon lien » n'est pas cochée
- **Candidature** : après l'envoi, l'écran Ma candidature affiche en tête « Ton lien personnel », avec la
  même puce et le même avertissement, puis le statut.
- **Sur cet appareil** (Accueil) : la liste locale des événements organisés et des candidatures, avec
  l'instance, la date et le rôle (« Organisateur » ou « Candidat »). Pour chaque entrée, le bouton
  « Oublier » ne retire que l'entrée locale.
- **Changer d'appareil** : « Copier mon lien » sur `/o` et `/p`, puis coller sur l'autre appareil. Rien
  d'autre ne permet de changer d'appareil, et il n'existe aucune récupération (verrou Q2).
- Le secret ne s'affiche que là où on l'a demandé. Il n'apparaît jamais dans un message Discord, un titre
  de page ni un aperçu de partage.

## Formulaires

**Création** : sur un seul écran, en moins de 60 s.

- Champs dans l'ordre : instance, région, portée de faction (Toutes / une faction), puis les créneaux.
- Chaque créneau se saisit par date et heure natives avec une durée, et « + Ajouter un créneau » en ajoute
  un.
- Viennent ensuite une note de 280 caractères maximum, le lien Discord, et « Lister publiquement » coché
  par défaut.
- Les bornes par classe et le CP minimum sont regroupés sous « Options », replié, avec les valeurs par
  défaut de la référence
- « Dupliquer » préremplit tout sauf les dates.

**Candidature** : sur un seul écran, en moins de 30 s.

- Champs : pseudo, serveur (liste groupée par faction, filtrée par région et par portée), classe, CP en
  nombre.
- Puis une bascule de disponibilité par créneau, avec ☆ pour la préférence.
- La faction et le rôle s'affichent en lecture seule une fois le serveur et la classe choisis.
- Par défaut, le dernier pseudo, le serveur et la classe utilisés sur cet appareil sont repris


## Inspiration & Anti-patterns

- **Repris des sondages sans compte (Framadate, Rallly)** : le lien est l'identité, et on candidate sans
  inscription.
- **Repris des bots d'inscription de raid sur Discord** : le résumé par rôle en une ligne (« manque
  1 Soin »).
- **Rejeté, la grille large de When2meet** : elle défile horizontalement sur téléphone, ce qui contredit
  AD-19.
- **Rejeté, le glisser-déposer pour composer** : il est fragile au doigt sur 360 px, demanderait une
  bibliothèque et serait inaccessible au clavier.
- **Rejeté, la fiabilité ou les « no-show » affichés** : verrou Vitrine.

## Responsive & Platform

- 360 px est la référence. De 320 à 359 px, la vue par créneau est forcée et la gouttière descend à
  12 px
- Au-delà de 640 px, la colonne est centrée. La matrice peut accueillir jusqu'à 7 créneaux sur PC si la
  largeur le permet, toujours selon la règle de largeur
- Les sélecteurs de date et d'heure sont ceux du navigateur, et le presse-papiers utilise l'API
  Clipboard, avec un repli par sélection.

## Key Flows

Les noms ci-dessous sont fictifs.

### Flux 1 : monter le raid (Kael, officier, mardi 20:40, téléphone, Discord ouvert)

1. Kael ouvre l'outil et touche « Créer un raid ».
2. Il choisit Ludra, Europe, Toutes factions, ajoute trois créneaux (mer., jeu. et sam. 21:00), puis
   touche « Publier ».
3. L'écran Liens créés s'ouvre. Il copie son lien d'organisateur dans ses messages privés Discord.
4. Il copie le lien direct pour les cinq habitués de la légion et le lien public pour le salon LFG.
5. **Climax** : 55 secondes après l'ouverture, les deux liens sont postés et Kael tient son lien
   d'organisateur. L'outil n'a demandé ni compte ni email.

Échec : la création est en pause. Le message le dit, et Kael poste « on fait à l'ancienne » sur Discord.

### Flux 2 : candidater depuis un autre serveur (Nyra, joueuse asmodienne, téléphone, pendant une file)

1. Nyra ouvre le lien public depuis le salon LFG.
2. Elle lit Ludra, trois créneaux, « manque 2 Soin », Toutes factions.
3. Elle saisit son pseudo, choisit son serveur (la faction s'affiche), Clerc, et son CP.
4. Elle coche jeudi et samedi, puis met ☆ sur les deux : les deux lui vont aussi bien.
5. Elle touche « Candidater ». L'écran Ma candidature s'ouvre sur « Ton lien personnel ».
6. **Climax** : le statut affiche « En attente », et la ligne de Nyra est apparue chez Kael en moins de
   10 secondes.

### Flux 3 : lire la grille, verrouiller, composer (Kael, mercredi 19:00, téléphone)

1. Kael ouvre son raid depuis « Sur cet appareil », onglet Candidatures. Il accepte Nyra et quatre
   autres, et refuse un doublon.
2. Il passe à l'onglet Disponibilités, en matrice (trois créneaux). Le pied de colonne affiche jeu. 14,
   sam. 9 et mer. 6.
3. Il touche « Verrouiller », retient jeudi et samedi avec 1 groupe chacun, puis touche « Verrouiller
   2 groupes ».
4. Il passe à l'onglet Composition et touche « Composer automatiquement » avec l'objectif « Équilibrer
   les CP ».
5. La proposition affiche G1 à 10 sur 10 et G2 à 8 sur 10, avec « manque 1 Soin ».
6. Il échange deux DPS entre G1 et G2 (toucher, « Échanger », toucher), puis touche « Appliquer ».
7. Il passe à l'onglet Salle et copie le message Discord, qu'il colle dans le salon du raid.
8. Jeudi 20:55, il copie le nom de la salle de G1, crée la salle en jeu avec Faction : All, et vérifie
   chaque `Pseudo[SRV]` à la demande de rejoint.
9. **Climax** : la salle est pleine, ses dix slots correspondent exactement à la carte de l'outil, et le
   raid part.

Échec : une candidature change pendant le brouillon. Le bandeau prévient Kael, qui choisit « Voir » ou
enregistre quand même.

### Flux 4 : un absent, le banc joue (Toma, remplaçant, jeudi 18:00)

1. Mercredi, Kael demande une confirmation aux titulaires de G1, avec l'échéance « 2 h avant ».
2. Toma, sur le banc, ouvre son lien et lit « Banc · G1 · jeu. 21:00 ». Il n'a rien à confirmer.
3. À 19:00, un titulaire est « expiré ». Kael ouvre Composition, déplace Toma du banc au slot libéré et
   touche « Enregistrer ».
4. Toma rouvre son lien et lit « Titulaire · Équipe 2, slot 4 · Soutien », ainsi que le nom de la salle.
5. **Climax** : à 21:00 la salle est pleine, sans qu'aucun message n'ait été envoyé par l'outil.

### Flux 5 : changer d'appareil (Nyra, du téléphone au PC)

1. Sur `/p`, Nyra touche « Copier mon lien » et se l'envoie en message privé sur Discord.
2. Elle l'ouvre sur son PC. La page Ma candidature s'affiche, et le lien est mémorisé sur ce navigateur.
3. **Climax** : elle modifie ses créneaux depuis le PC, sans compte.

## Couverture des besoins

| Besoin (source) | Surface | Flux |
|---|---|---|
| CAP-1 créer, liens, dupliquer | Créer, Liens créés, Réglages | 1 |
| CAP-2 candidater sans compte | `/e`, `/j` | 2 |
| CAP-3 grille, verrouiller, placer | Disponibilités, Composition | 3 |
| CAP-4 liste filtrable | Accueil | 2 (par lien) : pas de flux dédié à la liste |
| CAP-5 modifier, retirer, annuler | `/p`, Réglages | 5 |
| COMP solveur ou main | Composition | 3, 4 |
| ROOM recette en jeu | Salle | 3 |
| RECONF re-confirmation | Salle, `/p` | 4 |
| CONTACT quatre gestes | puces à copier, message Discord, lien Discord des acceptés, `/p` | 3, 4 |
| ADMIN | `/a` | aucun : écran minimal |
| I18N | feuille Langue · Thème | tous |

## Questions ouvertes

- Les noms français des classes et des instances sont à relever dans le client : ils vont dans la
  référence, pas ici.
- Le gabarit de `ROOM_NAME_TEMPLATE` et sa longueur maximale en jeu conditionnent la taille `display` du
  nom de salle.
- La longueur maximale d'un pseudo en jeu conditionne la troncature dans la colonne de 136 px.
- La règle du placement manuel hors disponibilité ou hors bornes est à confirmer en spec (voir *Écran
  de composition*).
