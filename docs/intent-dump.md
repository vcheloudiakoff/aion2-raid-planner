# Intent Dump — aion2-raid-planner

> 05/11/2026 — Vincent. Rédigé à partir de ses mots (§1 à §6, §9 : l'intention, reformulée au plus près)
> et d'une courte recherche web du même jour (§7 et `docs/domaine-aion2.md` : sourcée, datée, à revérifier).
> Destiné à `/bmad-forge-idea`, puis `/bmad-product-brief` et `/bmad-architecture` (un chat neuf par skill, depuis ce dépôt).
> Statut : **brouillon**. Marqueurs : `[À REMPLIR]` trou connu, laissé vide exprès (le skill le remontera
> en assumption ou en question plutôt que d'inventer) ; `[ASSUMPTION]` hypothèse à confirmer par Vincent ;
> `[PROPOSITION]` piste technique proposée par l'assistant, qui n'est pas une décision.

## 1. Pourquoi (douleur)

- Monter un raid dans Aion 2 (Sanctuaire : 10 joueurs) demande de trouver neuf autres joueurs disponibles
  au même moment. Le jeu permet de jouer ces instances inter-faction et inter-serveur, donc le vivier est
  large, mais rien n'aide à s'y retrouver : les gens se cherchent par messages épars, et personne ne voit
  qui est disponible quand. `[ASSUMPTION]` : aujourd'hui cela passe par Discord et le chat de légion.
- Pourquoi maintenant : le jeu est sorti en Europe le 5 octobre 2026, le raid Sanctuaire Ludra est revenu
  sur les serveurs globaux le 4 novembre 2026 (voir `docs/domaine-aion2.md`). La communauté se forme en
  ce moment, les habitudes d'organisation ne sont pas encore prises.
- Si on ne fait rien : des raids qui ne se montent pas, des annulations faute de monde, du temps perdu à
  relancer dans trois salons.

## 2. Pour qui

- Utilisateur : des joueurs d'Aion 2, région Europe, francophones d'abord `[ASSUMPTION]`. Deux rôles :
  l'**organisateur** qui veut remplir son raid et fixer une date, le **participant** qui cherche un groupe
  pour un créneau qui lui convient. Acheteur et payeur : personne, outil gratuit hébergé par Vincent.
- Contexte d'usage : entre deux sessions de jeu, souvent sur **mobile** avec Discord ouvert à côté, ou en
  jeu sur PC en alt-tab. Usage fréquent mais très court : quelques secondes pour regarder, une minute pour
  s'inscrire. Niveau d'expertise : joueurs, pas des techniciens ; toute friction (créer un compte,
  confirmer un email) fait fuir. C'est la raison du « pas de login ».
- Ce qu'ils font aujourd'hui à la place : salons LFG sur Discord, chat en jeu, matchmaking automatique du
  jeu (qui compose un groupe au hasard à l'instant, mais ne planifie rien). `[ASSUMPTION]`
- Taille du public visé (une légion, un serveur, la communauté FR entière) : `[À REMPLIR]` — change le
  besoin de modération et de découvrabilité (Q4, Q9).

## 3. Ce qui doit être vrai à la fin (capacités, WHAT pas HOW)

- **CAP-1** — Un organisateur publie un raid (instance, créneaux candidats, places par rôle, factions et
  serveurs acceptés, courte note) et obtient un lien partageable, **sans créer de compte**.
  *Succès* : création en moins de 60 s sur mobile ; le lien se copie en un geste ; aucun formulaire
  d'inscription préalable.
- **CAP-2** — Un joueur rejoint depuis le lien ou depuis la liste des raids ouverts : pseudo, serveur,
  faction, classe ou rôle, et coche ses disponibilités sur les créneaux proposés, **sans compte**.
  *Succès* : moins de 30 s ; sa ligne apparaît aussitôt chez l'organisateur.
- **CAP-3** — L'organisateur voit sur un écran qui est disponible à quel créneau, verrouille la date, et
  la composition (places et rôles pourvus ou manquants) ; les inscrits voient la date retenue.
  *Succès* : la grille joueurs × créneaux se lit sur mobile sans défilement horizontal ; verrouiller la
  date est un geste.
- **CAP-4** — Un joueur sans groupe trouve les raids ouverts qui lui correspondent : instance, faction,
  serveur, créneau, rôle encore manquant.
  *Succès* : filtrer par instance et par rôle manquant donne une liste utile en moins de 2 s.
- **CAP-5** — Un inscrit modifie ou retire sa participation ; l'organisateur modifie ou annule son raid ;
  personne d'autre ne peut le faire à leur place, toujours sans compte.
  *Succès* : depuis le même navigateur, immédiat ; depuis un autre appareil, possible par un mécanisme
  simple à trancher (Q2).
- **CAP-6** — Les données de référence du jeu (régions, serveurs par faction, instances, classes et rôles)
  sont justes et modifiables sans redéploiement.
  *Succès* : ajouter un serveur revient à éditer une donnée, visible sans reconstruire l'application.
- **CAP-7** — Les raids passés disparaissent d'eux-mêmes de la liste et leurs données sont purgées après
  un délai court.
  *Succès* : aucun raid de plus de N jours `[À REMPLIR]` dans la liste ni en base.

Hors v1 mais envisagés : notifier les inscrits quand la date est verrouillée (Q3) ; proposer les
disponibilités récurrentes d'un joueur (« dispo les mardis et jeudis soir ») pour que les organisateurs
le trouvent.

## 4. Ce qui ne doit PAS changer / contraintes qui plient le design

- **Hébergement imposé** : le NAS personnel de Vincent, déjà derrière un reverse proxy Caddy (TLS
  automatique) et du Docker Compose, avec un runner GitHub Actions auto-hébergé pour déployer sur push.
  Exclut tout hébergeur externe payant pour la v1.
- **Sécurité, non négociable** : le service est public, il ne doit jamais devenir un tremplin vers le NAS.
  Conteneur non-root, jamais `privileged`, jamais `pid: host`, jamais le socket Docker, aucun montage de
  répertoire du NAS en dehors de son propre volume de données, réseau Docker dédié, limites CPU et mémoire,
  secrets hors dépôt. Un autre service du NAS tourne aujourd'hui en privilégié avec accès au socket Docker :
  **ce motif est explicitement interdit ici**. Exclut toute fonctionnalité qui demande d'accéder à l'hôte.
- **Pas de login, pas de mot de passe, pas d'email**. Exclut les comptes classiques, et donc aussi la
  récupération par email. L'identité doit tenir sur autre chose (§8, Q1, Q2).
- **Données minimales** : pseudo de jeu, serveur, faction, classe, disponibilités. Pas d'email, pas de
  profil, pas de traceur tiers. Adresses IP utilisées pour l'anti-abus seulement, conservées au plus
  `[À REMPLIR]` (proposition : 24 h). Exclut tout analytics externe.
- **Rapide, mobile d'abord** : pages légères, utilisable sur réseau mobile médiocre. Cible chiffrée
  `[À REMPLIR]` — proposition `[ASSUMPTION]` : premier affichage utile en moins d'une seconde en 4G.
- **Solo, bénévole, 0 € récurrent** au-delà de ce qui existe (domaine, NAS, électricité). Exclut tout
  composant qui demande de l'exploitation régulière ou un service managé payant.
- **Dépôt public** : le code est sur GitHub en public. Exclut tout secret, adresse IP, nom d'hôte du NAS
  ou donnée de joueur dans le dépôt.
- **Fuseaux** : public européen mais pas uniquement CET ; stocker en UTC, afficher dans le fuseau du
  navigateur.
- **Langue** : français d'abord. Anglais : Q10.

## 5. Non-objectifs (explicites)

- Pas de comptes ni de mots de passe ; pas de profil joueur persistant inter-appareils en v1 au-delà du
  mécanisme retenu pour CAP-5.
- Pas d'intégration avec le jeu : aucune API publique NCSoft connue `[À VÉRIFIER]`, pas de lecture de
  personnages ni d'équipement.
- Pas de chat ni de messagerie : Discord fait ça, le lien du raid y sera posté.
- Pas de gestion de légion, de DKP ou de loot, pas de planning PvP (Abysse, sièges, forteresses).
- Pas de matchmaking automatique : l'outil aide des humains à se trouver, il ne compose pas les groupes.
- Pas d'application native (stores) : web mobile, éventuellement installable (PWA) plus tard.
- Pas de multi-jeu en v1 (mais ne pas écrire « Aion 2 » en dur partout : question d'architecture, pas de
  produit).

## 6. Signal de succès

- Un vrai raid Sanctuaire à 10 (par exemple Ludra) organisé de bout en bout avec l'outil : annonce
  publiée → au moins neuf inscrits venus d'au moins deux serveurs, idéalement des deux factions → date
  verrouillée à partir de la grille des disponibilités → raid joué. Et personne n'a demandé
  « comment je me connecte ? ».
- Et une relecture de la stack déployée (compose, Caddy) ne trouve aucun des points interdits du §4.

## 7. Existant

Greenfield : pas de code. Mais une infrastructure existe et des motifs sont déjà établis sur le NAS :

- À réutiliser : Caddy comme unique point d'entrée TLS ; une stack Docker Compose par service ; déploiement
  par push via le runner GitHub Actions auto-hébergé (à confirmer pour ce projet, Q7).
- À rompre volontairement : le backend privilégié avec socket Docker monté (§4).
- À regarder : un autre projet de Vincent place un VPS en bord de réseau devant le NAS pour ne pas exposer
  l'adresse IP du domicile. Le domaine du NAS actuel pointe, lui, directement sur la box (Q5).
- Compétences disponibles chez Vincent (indication pour l'architecture, pas une contrainte) : Rust/Axum +
  SQLx + PostgreSQL, React/TypeScript/Vite/Tailwind, Elm/Lamdera, Astro.
- Connaissance du domaine rassemblée dans `docs/domaine-aion2.md` : factions, régions, serveurs, instances
  et tailles de groupe, classes. À vérifier dans le client avant d'en faire des données de référence.

## 8. Décisions déjà prises (et pourquoi) / pistes proposées

Décisions de Vincent :

- Hébergement sur le NAS perso (gratuit, déjà en place) ; donc sécurité d'abord.
- Pas de login : la friction fait fuir des joueurs qui veulent juste trouver un groupe.
- Simple, rapide, efficace : trouver des gens, se mettre d'accord sur une date, point.
- Inter-faction et inter-serveur dès le départ, parce que le jeu le permet pour ces instances.
- Dépôt public sur GitHub ; méthode BMAD (v6.12) pour planifier et construire.

Pistes `[PROPOSITION]`, à tester par `forge-idea` puis à trancher dans la spine d'architecture :

- **Identité sans compte par liens porteurs de droits** : un raid = une URL publique de lecture et une URL
  (ou jeton) secrète d'organisateur ; une inscription = un jeton participant gardé par le navigateur ;
  option « code de reprise » à recopier pour changer d'appareil. Précédents : Framadate, Rallly,
  When2meet, Doodle sans compte.
- **Anti-abus sans compte** : limitation de débit par adresse au niveau de Caddy et de l'application,
  tailles de champs bornées, texte libre très court, honeypot, défi humain (type Turnstile) uniquement à
  la création d'un raid si le spam apparaît, expiration automatique (CAP-7). Le périmètre « une communauté
  qui se connaît » réduit beaucoup le risque (Q9).
- **Isolation sur le NAS** : stack Compose dédiée, image minimale, `read_only`, `cap_drop: ALL`,
  `no-new-privileges`, utilisateur non-root, limites mémoire et CPU, port exposé seulement au reverse
  proxy ; en-têtes de sécurité (CSP stricte, pas de script tiers). Base : SQLite dans un volume dédié
  (une seule chose à sauvegarder, zéro service de plus) plutôt que de partager le PostgreSQL d'un autre
  service (couplage, et une brèche en toucherait deux).
- **Exposition de l'adresse du domicile** : réutiliser le motif VPS en bord de réseau déjà en place, ou un
  proxy type Cloudflare, plutôt que d'ouvrir un nouveau sous-domaine direct vers la box (Q5).
- **Visibilité** : liste publique des raids par défaut (c'est ce qui permet de trouver du monde), avec une
  option « lien seul » par raid (Q4).
- **Notifications** : aucune en v1 ; la boucle de retour reste Discord, où le lien est posté. Plus tard,
  webhook Discord par raid ou Web Push, sans email (Q3).

## 9. Organisation

- Vincent seul : produit, design, technique, exploitation. Les cinq sign-offs BMAD sont les siens.
- Dépôt : `github.com/vcheloudiakoff/aion2-raid-planner` (public). Branche `main`.
- Déploiement : `[À TRANCHER]` runner auto-hébergé sur push (comme les autres services du NAS) ou manuel
  au début (Q7).
- Cadence, temps disponible par semaine : `[À REMPLIR]`.
- Qui d'autre doit être d'accord : personne. Qui testera en vrai : `[À REMPLIR]` (sa légion ? des joueurs
  rencontrés en jeu ?).

## 10. Risques, inconnues, questions ouvertes

- **Q1** — Sans compte, comment tenir le spam et le vandalisme (faux raids, inscriptions fantômes) à un
  niveau acceptable ? Jusqu'où aller en v1 ?
- **Q2** — Comment un joueur retrouve-t-il ses droits (modifier son inscription, son raid) depuis un autre
  appareil, sans email ? Code de reprise, QR, rien en v1 ?
- **Q3** — Faut-il notifier (date verrouillée, raid annulé) ? Par quel canal sans email ?
- **Q4** — Liste publique ou liens seuls ? Découvrabilité contre spam et contre « le raid de ma légion
  envahi par des inconnus ».
- **Q5** — Exposer un nouveau sous-domaine direct vers la box, ou passer par le VPS déjà en place ?
  Quelle est la menace réelle visée : la découverte du NAS, ou sa compromission par ce service ?
- **Q6** — Un groupe prédéfini inter-serveur peut-il réellement entrer ensemble dans un Sanctuaire, ou le
  jeu ne permet-il l'inter-serveur que par matchmaking ? Vincent dit oui ; à confirmer dans le client,
  cela conditionne la promesse centrale.
- **Q7** — Déployer par le runner existant (un push = prod) ou à la main tant que c'est un pilote ?
- **Q8** — Les listes de serveurs varient selon les sources et NCSoft en ajoute : qui met à jour les
  données de référence, à quelle occasion ?
- **Q9** — Public visé : la légion de Vincent, son serveur, la communauté FR ? Décide de l'échelle, de la
  modération et du ton.
- **Q10** — Français seulement, ou anglais aussi (les serveurs EU sont multilingues) ?
- **Q11** — Stack : rien n'est imposé. Critère : le plus petit nombre de pièces à sécuriser et à
  maintenir seul, dans une techno que Vincent pratique déjà.

## 11. Matériaux joints

- `docs/domaine-aion2.md` : connaissance du jeu utile au produit, avec sources et dates.
- Dépôts locaux de référence (non publics, à lire sur la machine de Vincent) : le dépôt du NAS pour Caddy,
  Compose et le runner ; le dépôt du projet qui utilise le VPS en bord de réseau pour ce motif.
- Pack de méthode : `~/bmad-method/` (cheatsheet, tutoriel, kickoff, automatisation).

## 12. Addendum post-forge (05/11/2026)

Le forge (`/bmad-forge-idea`, mode attaque) a rendu le verdict **HARDENED**. Le distillat fait foi :
`_bmad-output/forge/aion2-raid-sans-compte/forged-idea.md` (journal : `.memlog.md`, rapport :
`forge-report.html`). Il remplace ce qui précède sur les points suivants :

- **§2** : public v1 = la légion de Vincent et son cercle (≈15 actifs par jour, ≤120 membres) ; le public
  du site est un pari, pas une promesse.
- **§3** : CAP-2 ajoute le **CP déclaré** et une préférence de créneau ; CAP-3 devient un événement à N
  créneaux qui produit K groupes (titulaires + banc), avec **composition** par solveur ou manuelle et
  **re-confirmation** par lien ; CAP-1 produit aussi un nom de salle unique et les 10 slots à régler en jeu.
- **§4** : les données minimales incluent le CP (donnée de jeu, pas personnelle) ; Discord uniquement au
  niveau du raid, posé par l'organisateur ; interface livrée en français **et** en anglais.
- **§5** : « pas de matchmaking automatique » se lit « pas d'assemblage d'inconnus au hasard » ; la
  composition d'un vivier connu est dans la v1. Aucune fiabilité, réputation ni historique, jamais.
- **§8 et §10** : Q1, Q2, Q3, Q4, Q5, Q6, Q9, Q10 sont tranchées dans le distillat ; Q7, Q8, Q11 vont
  à la spine d'architecture avec leur critère.
