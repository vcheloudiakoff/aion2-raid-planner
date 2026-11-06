# Réconciliation : `docs/domaine-aion2.md` contre `ARCHITECTURE-SPINE.md`

> Revue du 2026-11-05. Source : `docs/domaine-aion2.md`. Cible : `ARCHITECTURE-SPINE.md` (draft du même jour).
> Portée : le modèle de données de référence (AD-6) et le modèle d'événement (AD-4, AD-13, AD-18, ROOM)
> peuvent-ils représenter ce que le domaine décrit, sans rien coder en dur ? Ce qui est bien couvert
> (un serveur = une faction, faction déduite du serveur, référence éditable rechargée sans rebuild,
> libellés bilingues, validation contre le jeu courant, durée par défaut de l'instance) n'est pas repris.

## Verdict

La spine prévoit la bonne mécanique (référence TOML éditable, validée, rechargée) mais ses invariants
figent à « dix par groupe » et « bornes par classe » ce que le domaine décrit comme variable par instance
et par rôle, et l'agrégat événement ne porte ni région ni portée de faction : c'est réconciliable par
retouches ciblées d'AD-4, AD-6, AD-13 et AD-18, sans réécrire la spine, mais deux retouches touchent des
verrous du forge et demandent l'arbitrage de Vincent.

## Constats

### 1. La taille de groupe est un invariant (« dix ») alors que la spine elle-même la déclare donnée de référence — haute

- **Source :** trois types d'instances de tailles différentes : expédition 5 (4 avant le chapitre 1, certaines
  en solo ou à 2 minimum), transcendance 5, sanctuaire 10. Les tailles ont déjà changé une fois (4 → 5,
  8 → 10). `AGENTS.md` et l'intent dump cadrent le produit sur « un raid (Sanctuaire) ou une expédition ».
- **Spine :** AD-6 met « taille de groupe » dans `instances.toml`, mais AD-4 fixe « dix `assignments`
  titulaires indexés 0..9 », AD-13 impose « dix par groupe » en contrainte dure, ROOM parle de « dix slots »
  et AD-18 génère la salle pour dix. La référence et les invariants se contredisent : un événement
  d'expédition est irreprésentable, et un changement de taille par NCSoft exigerait un rebuild.
- **Où :** AD-4 (Rule), AD-13 (contraintes dures), AD-18, Capability Map ligne ROOM.
- **Sévérité :** haute. Touche le verrou du forge « 10 titulaires / 10 par groupe », qui a été écrit en
  pensant Sanctuaire ; le minimum compatible avec le verrou est de garder 10 comme *valeur* du Sanctuaire
  dans la référence, pas comme *invariant* du domaine.
- **Correction :** remplacer partout « dix » par « `group_size` de l'instance de l'événement (référence) »,
  indices `0..group_size-1`, contrainte dure « `group_size` par groupe », et faire porter à `instances.toml`
  un `kind` (expedition / transcendence / sanctuary) utile au filtre CAP-4.

### 2. Le rôle n'existe ni dans la candidature, ni dans les bornes, ni dans les slots — haute

- **Source :** « une inscription doit pouvoir indiquer le rôle (tank, soin, DPS) autant que la classe » ;
  une neuvième classe non confirmée ; le rôle « habituel » d'une classe n'est pas unique (Aède : soutien
  et soins secondaires ; Spiritualiste : DPS et invocateur). L'intent dump dit « places par rôle » (CAP-1),
  « classe ou rôle » (CAP-2), « rôle encore manquant » (CAP-4) ; le forge dit « rôle par slot ».
- **Spine :** le rôle n'apparaît que dans la référence (`classes` avec rôle, `roles`). L'entrée de
  `compose()` est « classe, CP, bornes par classe » ; l'entité est `class_bounds` ; aucun `role` sur la
  candidature, le slot ou l'assignment. Un organisateur ne peut donc pas dire « il me manque un soigneur »,
  et un joueur ne peut pas s'inscrire « Clerc en DPS ». Les « bornes par défaut » sont accrochées à la
  classe (AD-6) alors qu'elles dépendent de l'instance (1 tank / 1 soin / 3 DPS à 5, 2 / 2 / 6 à 10).
- **Où :** AD-4 (contenu de la candidature, `class_bounds`), AD-6 (classes : « bornes par défaut »), AD-13
  (entrée et contraintes), Conventions (table `class_bounds`), diagramme ER (`CLASS_BOUND`).
- **Sévérité :** haute. Touche le verrou du forge « bornes min/max par classe » ; le domaine et trois CAP
  de l'intent dump pointent vers le rôle, le forge vers la classe : Vincent arbitre.
- **Correction :** la candidature porte `class` et `role` (rôle proposé par défaut depuis la classe via la
  référence, modifiable par le joueur), les bornes de l'événement sont par rôle avec bornes par classe
  optionnelles (`role_bounds` + `class_bounds`, ou une table `bounds` à clé `role|class`), et les bornes
  par défaut vivent dans `instances.toml` par rôle, pas dans `classes.toml`.

### 3. L'agrégat événement n'a pas de région ; la référence ne lie pas serveur → région — haute

- **Source :** cinq régions ; choix à la création « région → faction → serveur » ; les serveurs sont listés
  « par région et par faction » ; le contenu instancié est inter-serveur mais rien n'indique qu'il soit
  inter-région ; la liste des salles de Sanctuaire est régionale (forge, Q6).
- **Spine :** `regions.toml` existe (AD-6, seed), mais `servers` sont décrits « avec leur faction » sans
  région ; l'événement (AD-4) n'énumère que ses collections (slots, candidatures, bornes, groupes) et jamais
  ses attributs propres (instance, région, portée de faction, serveurs acceptés, note, lien Discord, « lien
  seul »). Sans région sur l'événement, la liste publique mélange les régions et la validation de
  candidature ne peut pas refuser un serveur d'une autre région (joueurs qui ne se rencontreront jamais).
- **Où :** AD-4 (Rule), AD-6 (schéma des serveurs), AD-14 (liste publique), CAP-4.
- **Sévérité :** haute.
- **Correction :** `servers.toml` porte `region` et `faction` (plus un `short` pour le `[SRV]` de
  `Pseudo[SRV]`, à relever dans le client), l'événement porte `region` et `instance` comme attributs validés
  contre la référence, et AD-4 ajoute l'invariant « le serveur d'une candidature appartient à la région de
  l'événement ».

### 4. L'inter-faction est une propriété de l'instance et une option de l'événement, pas une constante — moyenne

- **Source :** sur la version globale les deux factions font équipe dans les donjons ; une option permet de
  ne matcher qu'avec sa faction ; « l'inter-faction ne s'applique qu'au contenu JcE qui le prend en
  charge : vérifier les options de groupe de l'activité ». Le forge verrouille « Faction : All » pour la
  salle de Sanctuaire ; l'intent dump met « factions et serveurs acceptés » dans CAP-1 et un filtre
  faction dans CAP-4.
- **Spine :** rien. Ni drapeau `cross_faction` sur l'instance, ni `faction_scope` (`all` | `elyos` |
  `asmodian`) sur l'événement, ni règle de validation « portée permise par l'instance ».
- **Où :** AD-6 (schéma des instances), AD-4 (attributs de l'événement), CAP-1, CAP-4.
- **Sévérité :** moyenne : le Sanctuaire v1 fonctionne en « All », mais une instance mono-faction ou un
  organisateur « légion, même faction » n'est pas représentable.
- **Correction :** `instances.toml` porte `cross_faction = true|false` (à vérifier dans le client par
  instance), l'événement porte `faction_scope` validé contre ce drapeau, et la candidature est refusée
  (code `candidature.faction_out_of_scope`) si la faction déduite du serveur sort de la portée.

### 5. La structure interne du groupe (deux groupes de 5, rôle par slot) est codée dans l'index 0..9 — moyenne

- **Source :** sanctuaire = « 10 joueurs, soit deux groupes de 5 » ; expédition = un seul groupe de 5. Le
  forge rend « les 10 slots (deux groupes de 5, rôle par slot) ».
- **Spine :** dix assignments à plat, indexés 0..9 ; le découpage en deux équipes de 5 et le rôle attendu
  par slot ne sont ni dans la référence ni dans l'agrégat : ils seraient déduits de l'index par une
  constante (`index / 5`) dans le code.
- **Où :** AD-4 (`assignments` 0..9), AD-18, ROOM.
- **Sévérité :** moyenne.
- **Correction :** `instances.toml` décrit la disposition (`parties = 2`, `party_size = 5`, ou une liste de
  slots avec rôle par défaut) et l'assignment porte `party` et `index` dérivés de cette disposition, le
  rôle attendu par slot venant des bornes de l'événement (constat 2).

### 6. Cycle de vie des entrées de référence (retrait, renommage, id incertain) face aux données vivantes — moyenne

- **Source :** Ludra retiré au lancement puis revenu le 4 novembre ; appariements et listes de serveurs
  « temporaires, ajustables » ; nom anglais de la classe incertain (Spiritmaster ou Elementalist) ; tout est
  « à vérifier dans le client ».
- **Spine :** « identifiants stables en anglais » (AD-6) sans dire que l'id est découplé du libellé `en` ;
  « le serveur valide chaque événement et chaque candidature contre le jeu courant » sans préciser si c'est
  à l'écriture seulement ou aussi à la lecture. Si un id disparaît du TOML (instance retirée, serveur
  fusionné, classe renommée), un événement vivant qui le référence devient-il illisible, non modifiable,
  ou purgé ? Rien ne le dit, et le rechargement à chaud rend le cas probable.
- **Où :** AD-6 (identifiants, validation), AD-16 (revalidation à chaque commande).
- **Sévérité :** moyenne.
- **Correction :** préciser qu'un id est un slug opaque jamais renommé (le libellé `en` peut changer),
  qu'une entrée se retire par `enabled = false` et non par suppression, que la validation contre la référence
  s'applique aux écritures qui *choisissent* une valeur (création, candidature, changement de classe) et
  qu'une lecture tolère un id absent avec repli d'affichage sur l'id.

### 7. Le nom de salle est composé côté serveur depuis un « libellé court » bilingue — moyenne

- **Source :** les noms français des instances sont « à relever dans le client » ; le nom de salle est
  saisi en jeu par l'organisateur et cherché par son nom par les neuf autres joueurs, quelle que soit leur
  langue d'interface.
- **Spine :** AD-18 compose `room_name` côté serveur depuis le « libellé court de l'instance », alors que
  AD-6 ne connaît que des libellés `fr` et `en` et qu'AD-12 interdit au serveur de choisir une langue. Le
  serveur devrait donc choisir une langue, ou produire deux noms de salle pour un même groupe.
- **Où :** AD-18, AD-6, AD-12.
- **Sévérité :** moyenne.
- **Correction :** `instances.toml` porte un `short` unique et neutre (`LUDRA`), distinct des libellés
  `fr`/`en`, seul utilisé par `ROOM_NAME_TEMPLATE`, avec longueur et charset du champ « nom de salle » du
  client ajoutés à la liste des points à vérifier.

### 8. Les libellés `fr` sont exigés alors que la source les donne pour incertains — basse

- **Source :** les noms français des classes viennent d'Aion premier du nom, « à vérifier dans le client
  d'Aion 2 » ; les sanctuaires coréens ont des « traductions libres ».
- **Spine :** « libellés `fr` et `en` par entrée » (AD-6), validation « complète » du fichier : une entrée
  sans `fr` vérifié est soit refusée, soit livrée avec une traduction inventée.
- **Où :** AD-6.
- **Sévérité :** basse.
- **Correction :** rendre `en` obligatoire et `fr` optionnel avec repli sur `en` à l'affichage, pour qu'une
  entrée puisse exister avant que son nom français soit relevé dans le client.

### 9. L'appariement de serveurs n'a pas de place dans le produit et ne doit pas en avoir dans la référence — basse

- **Source :** chaque serveur Élyséen est apparié à un serveur Asmodien pour l'Abysse (JcJ) ; appariements
  temporaires et contradictoires selon les sources.
- **Spine :** silencieuse, ce qui est juste : le produit ne couvre que le contenu instancié inter-serveur,
  où l'appariement ne contraint rien. Le risque est qu'un contributeur l'ajoute au TOML comme une relation
  structurante et qu'une règle en dépende.
- **Où :** AD-6, `.memlog.md`.
- **Sévérité :** basse.
- **Correction :** noter dans le memlog que l'appariement est volontairement hors référence v1 ; s'il entre
  un jour, c'est un champ d'affichage optionnel, jamais une donnée de validation ni de composition.

### 10. « Un joueur dans un seul groupe » et « plusieurs raids par semaine » — basse

- **Source :** sanctuaire sans limite de tentatives, deux récompenses par semaine ; expéditions sans ticket ;
  « un joueur peut vouloir plusieurs raids par semaine ».
- **Spine :** la contrainte dure « un joueur dans un seul groupe » (AD-13, verrou du forge) vaut dans un
  événement ; rien ne lie les candidatures entre événements (pas de compte), donc plusieurs raids par
  semaine = plusieurs événements, ce qui marche. Deux effets de bord restent : un événement à K groupes sur
  des créneaux différents ne peut pas réutiliser un joueur disponible aux deux, et les quotas AD-7 « par IP »
  pour la candidature doivent laisser passer plusieurs candidatures par jour depuis une même IP (légion
  derrière une box, IPv6 /64) sans les prendre pour de l'abus.
- **Où :** AD-13 (contraintes dures), AD-7 (quotas).
- **Sévérité :** basse.
- **Correction :** préciser « un joueur dans un seul groupe *par créneau* » ou, si le verrou reste « par
  événement », l'écrire tel quel ; et fixer les quotas de candidature AD-7 avec cet usage légitime en
  référence (valeurs en configuration, pas dans la spine).

## Synthèse

| # | Constat | Sévérité | Touche un verrou du forge |
| --- | --- | --- | --- |
| 1 | Taille de groupe figée à dix | haute | oui (« 10 par groupe ») |
| 2 | Rôle absent de la candidature, des bornes et des slots | haute | oui (« bornes par classe ») |
| 3 | Pas de région sur l'événement ni sur les serveurs | haute | non |
| 4 | Inter-faction : drapeau d'instance + portée d'événement absents | moyenne | non |
| 5 | Deux groupes de 5 codés par l'index | moyenne | non |
| 6 | Cycle de vie des entrées de référence | moyenne | non |
| 7 | Nom de salle depuis un libellé bilingue | moyenne | non |
| 8 | Libellés `fr` exigés mais incertains | basse | non |
| 9 | Appariement de serveurs : à exclure explicitement | basse | non |
| 10 | Un joueur par groupe, plusieurs raids par semaine | basse | oui (« un joueur par groupe ») |

Les constats 1 et 2 sont ceux qui méritent une décision de Vincent avant `bmad-spec` : ils opposent le
domaine (et trois CAP de l'intent dump) à deux verrous du forge écrits en pensant Sanctuaire seul. Les
autres sont des précisions d'AD-4 et d'AD-6 qui ne changent aucune décision.
