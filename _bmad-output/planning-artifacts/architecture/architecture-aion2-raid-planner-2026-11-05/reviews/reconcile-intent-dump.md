# Réconciliation : `docs/intent-dump.md` → `ARCHITECTURE-SPINE.md`

> 05/11/2026. Source : `docs/intent-dump.md`, sections qui font foi §3, §4, §5, §6, §7, §9, lues à la
> lumière du §12 (ce que le forge a remplacé n'est pas signalé comme manquant ; le distillat
> `_bmad-output/forge/aion2-raid-sans-compte/forged-idea.md` a été consulté pour Q7, Q8, Q11).
> Cible : `ARCHITECTURE-SPINE.md` (draft du 2026-11-05). Seuls les écarts sont listés ; ce qui est bien
> couvert (isolation du conteneur, liens porteurs de droits, UTC, données minimales dans l'application,
> aucun appel sortant, fr + en, stack dans les technos que Vincent pratique) n'est pas répété.

## Verdict

La spine couvre fidèlement la sécurité non négociable, l'identité sans compte, les fuseaux et la stack,
mais aucun des critères de succès chiffrés du §3 ni la contrainte « rapide, mobile d'abord » du §4 n'y a
atterri, et deux paires d'AD se contredisent sur des exigences explicites de la source : AD-6/AD-10 sur
« modifiable sans reconstruire » (CAP-6, Q8) et AD-4/AD-13 contre AD-6 sur « rien d'Aion 2 en dur ».

## Constats

### C1 — « Rapide, mobile d'abord » et « premier affichage < 1 s en 4G » n'ont aucun AD — **haute**

- **Source** (§4, CAP-3) : pages légères, utilisable sur réseau mobile médiocre ; cible proposée
  `[ASSUMPTION]` premier affichage utile en moins d'une seconde en 4G ; grille joueurs × créneaux lisible
  sur mobile sans défilement horizontal. Le forge (point faible 5) ajoute que l'écran de composition sur
  mobile est « le plus gros risque de glissement ».
- **Spine** : rien ne borne le front. La ligne « Front » des Consistency Conventions fixe Tailwind et
  l'absence de bibliothèque d'état, AD-1 sert des assets hachés, AD-9 compresse à l'edge, mais aucun budget
  de poids, aucune règle de découpage, rien sur la largeur minimale ni sur le premier rendu (qui dépend
  aujourd'hui d'un aller-retour `GET /api/reference` avant tout formulaire). Le mot « mobile » n'apparaît
  pas dans la spine.
- **Concerné** : Consistency Conventions (Front), AD-1, AD-6, AD-9 ; aucune entrée « Deferred ».
- **Correction** : ajouter un AD « Front léger, mobile d'abord » qui fixe un budget (JS initial gzip
  plafonné, aucune dépendance front hors React/react-router, référence servie avec ETag et mise en cache
  côté client, premier rendu sans attendre `/api/reference`), la largeur de référence (360 px, sans
  défilement horizontal) comme contrainte transmise à `bmad-ux`, et le critère « < 1 s en 4G » comme
  `[ASSUMPTION]` à confirmer par Vincent.

### C2 — CAP-6 / Q8 : le seul chemin documenté reconstruit l'image et personne ne valide les TOML — **haute**

- **Source** (CAP-6, §12 Q8) : « ajouter un serveur revient à éditer une donnée, visible sans reconstruire
  l'application » ; le forge précise le critère remis à la spine : « fichiers de données éditables lus sans
  reconstruire l'image ; Vincent les met à jour après vérification dans le client ».
- **Spine** : AD-6 prévoit bien le rechargement par mtime de `/data/reference/`, mais conclut « Ajouter
  un serveur = éditer le TOML, commit, push », et le push déclenche AD-10 (`docker compose up -d --build`)
  : `reference/` est dans le contexte de build (l'image en embarque une copie), donc chaque changement de
  donnée reconstruit l'image et remplace le conteneur. Le rechargement à chaud n'a pas de porte d'entrée
  propre. De plus AD-10 (job `ci`) ne valide pas `reference/*.toml` : un fichier invalide donne, par AD-6,
  « ancien jeu conservé et erreur journalisée », avec un health check vert, donc une modification
  silencieusement ignorée. Enfin, ni AD-6 ni AD-10 ne reprennent « après vérification dans le client »
  (§7 : « à vérifier dans le client avant d'en faire des données de référence »).
- **Concerné** : AD-6, AD-10, Capability Map CAP-6.
- **Correction** : faire valider `reference/*.toml` en CI par une sous-commande du binaire
  (`check-reference`), faire que le job `deploy` ne copie que `reference/` dans le volume sans rebuild
  quand seul ce dossier a changé (ou documenter l'édition directe dans `/data/reference/` sur le NAS comme
  chemin nominal), exposer la version du jeu chargé dans `GET /api/health` pour que le déploiement
  constate la prise en compte, et noter dans AD-6 la règle « vérifié dans le client avant commit ».

### C3 — Dépôt public + runner sur le NAS : AD-10 ne borne pas les déclencheurs ni l'accès Docker — **haute**

- **Source** (§4) : le service « ne doit jamais devenir un tremplin vers le NAS » ; dépôt public sur
  GitHub ; (§7) « à rompre volontairement : le backend privilégié avec socket Docker monté ».
- **Spine** : AD-10 fait tourner le job `deploy` sur `[self-hosted, nas]` et exécute `docker compose up
  -d --build` (donc un accès Docker équivalent root sur le NAS) à partir d'un dépôt public, sans dire que
  seul `push` sur `main` déclenche (jamais `pull_request`, jamais un fork), sans restreindre le runner à
  ce dépôt, et sans dire comment le runner atteint Docker : s'il tourne en conteneur avec le socket monté,
  c'est précisément le motif « à rompre ». GitHub déconseille les runners auto-hébergés sur dépôt public.
- **Concerné** : AD-10, AD-8 (« Prevents »), Consistency Conventions « Dépôt public ».
- **Correction** : AD-10 fixe `on: push: branches: [main]` comme unique déclencheur du job self-hosted,
  un groupe de runners limité à ce dépôt, l'approbation obligatoire des workflows de contributeurs
  externes, et déclare (ou marque `[ASSUMPTION]`) que le runner est un processus hôte et non un conteneur
  privilégié avec socket.

### C4 — CAP-2 « sa ligne apparaît aussitôt chez l'organisateur » : aucun mécanisme de rafraîchissement — **moyenne**

- **Source** (CAP-2, succès) : la candidature apparaît aussitôt dans la vue organisateur.
- **Spine** : Front = « hooks locaux + fetch, sans bibliothèque » ; AD-16 renvoie l'agrégat à jour, mais
  seulement à l'auteur de l'écriture ; rien sur un polling, SSE ou WebSocket pour l'écran `/o`. Le choix
  n'est pas neutre : SSE touche `http`, les délais d'inactivité des deux Caddy (AD-9) et la CSP
  (`connect-src 'self'` suffit, mais il faut le savoir).
- **Concerné** : AD-5, AD-9, AD-16, Capability Map CAP-2/CAP-3.
- **Correction** : trancher dans AD-5 un rafraîchissement explicite (polling court de `organizer_view`
  avec ETag/304, ou un flux SSE `GET /api/events/:id/stream` sans effet de bord) et noter l'impact Caddy.

### C5 — « Ne pas écrire Aion 2 en dur partout » : la taille de groupe est codée « dix » — **moyenne**

- **Source** (§5) : pas de multi-jeu en v1, « mais ne pas écrire “Aion 2” en dur partout : question
  d'architecture ».
- **Spine** : AD-6 met « taille de groupe » dans la référence des instances, mais AD-4 fixe « dix
  `assignments` titulaires indexés 0..9 », AD-13 « dix par groupe » en contrainte dure, et la ligne ROOM
  « nom de salle et dix slots ». C'est une contradiction interne et une valeur propre au Sanctuaire
  gravée dans le noyau ; le découpage « deux groupes de 5, rôle par slot » du forge n'est pas davantage une
  donnée. Hors ce cas, aucune règle de la spine ne dit où un nom ou un nombre propre au jeu a le droit
  de vivre.
- **Concerné** : AD-4, AD-13, AD-18, Capability Map ROOM, AD-6.
- **Correction** : AD-4/AD-13/AD-18 lisent `group_size` (et le découpage en sous-groupes) depuis
  l'instance de référence, et la spine ajoute une règle : hors `reference/` et `locales/`, aucun nom ni
  nombre propre au jeu dans `domain`, `server` ou `web`.

### C6 — §6 « une relecture du compose et du Caddy ne trouve aucun point interdit » : aucune vérification automatique — **moyenne**

- **Source** (§6, §4) : signal de succès explicite sur la stack déployée.
- **Spine** : AD-8 fixe le contenu du compose et AD-17 a son test `cargo tree`, mais le job `ci` d'AD-10
  ne vérifie ni `deploy/docker-compose.yml` ni `deploy/caddy/`. La relecture reste humaine et ponctuelle,
  alors que `git reset --hard origin/main` rend le fichier du dépôt exactement celui qui est déployé.
- **Concerné** : AD-10, AD-8, AD-9.
- **Correction** : ajouter au job `ci` un test qui échoue si le compose contient `privileged`, `pid:`,
  `docker.sock`, `cap_add`, `network_mode: host` ou un bind mount, ou s'il manque `user`, `read_only`,
  `cap_drop`, `mem_limit`, `cpus`, le préfixe `127.0.0.1:` sur `ports` ; et un grep d'adresses/hôtes sur
  `deploy/caddy/`.

### C7 — Rétention IP ≤ 24 h : muette sur les journaux d'accès des deux Caddy — **moyenne**

- **Source** (§4) : adresses IP pour l'anti-abus seulement, conservées au plus 24 h (proposition).
- **Spine** : AD-7 et AD-15 règlent le cas dans l'application (limiteur en mémoire, journaux sans IP),
  mais AD-9 fixe ce que font les Caddy du VPS et du NAS sans rien dire de leurs journaux d'accès, seul
  endroit où l'IP reste visible ; le VPS est un edge partagé avec un autre projet.
- **Concerné** : AD-9, AD-7, AD-15.
- **Correction** : AD-9 ajoute « aucun journal d'accès (ou rotation ≤ 24 h) sur les blocs Caddy de ce
  site, VPS et NAS », et la question juridique du Deferred renvoie à cette ligne.

### C8 — « Création < 60 s » et « inscription < 30 s » : non portés, alors qu'AD-7 en consomme — **moyenne**

- **Source** (CAP-1, CAP-2) : création en moins de 60 s sur mobile, lien copiable en un geste ;
  inscription en moins de 30 s.
- **Spine** : les deux chiffres sont absents. AD-7 est le seul AD qui pèse sur ce budget : aller-retour
  `GET /api/form-token`, `MIN_SUBMIT_DELAY_MS` (3000), preuve de travail derrière `POW_ENABLED` sans
  budget de temps sur un mobile d'entrée de gamme. Le « copier en un geste » n'est renvoyé nulle part.
- **Concerné** : AD-7, Capability Map CAP-1/CAP-2, Deferred « Preuve de travail côté UI ».
- **Correction** : AD-7 fixe que le jeton est obtenu au chargement du formulaire et que la preuve de
  travail est bornée (quelques secondes sur mobile), et la Capability Map cite les critères 60 s / 30 s
  comme propriété à vérifier par le SPEC et l'UX.

### C9 — CAP-7 « aucun raid de plus de N jours en base » : les sauvegardes prolongent la vie des données — **basse**

- **Source** (CAP-7) : aucun raid de plus de N jours dans la liste ni en base ; N `[À REMPLIR]`.
- **Spine** : AD-14 purge après `PURGE_AFTER_DAYS` (7), AD-2 garde sept sauvegardes quotidiennes
  complètes : une donnée vit jusqu'à 14 jours ; AD-14 l'admet (« au-delà de la rotation »). Pour une
  base éphémère par construction, la sauvegarde est aussi une pièce de plus au sens du critère Q11.
- **Concerné** : AD-2, AD-14.
- **Correction** : définir N comme `PURGE_AFTER_DAYS` + rétention des sauvegardes et le dire dans AD-14,
  ou réduire la rétention des sauvegardes (deux copies), ou les supprimer.

### C10 — CAP-1 « factions et serveurs acceptés » et CAP-4 « filtrer par faction, serveur » : aucun attribut dans l'agrégat — **basse**

- **Source** (CAP-1, CAP-4) : l'organisateur fixe factions et serveurs acceptés ; un joueur filtre par
  faction et serveur. Le §12 ne retire pas ces champs (le forge dit seulement « la faction se déduit du
  serveur » et « inter-faction dès le départ »).
- **Spine** : AD-4 décrit `slots`, `candidatures`, `class_bounds`, `groups` ; rien sur les serveurs
  acceptés ; `list_public_events` n'a pas ces filtres.
- **Concerné** : AD-4, Capability Map CAP-1/CAP-4.
- **Correction** : déclarer dans AD-4 que le champ est abandonné (public = légion, inter-faction par
  défaut) ou ajouter `accepted_servers` à l'événement et au filtre public.

### C11 — CAP-4 « filtre < 2 s » et « rôle encore manquant » : lieu du calcul non fixé — **basse**

- **Source** (CAP-4) : filtrer par instance et par rôle manquant donne une liste utile en moins de 2 s.
- **Spine** : AD-11 dit que la liste publique se calcule côté serveur, AD-4 interdit des statuts calculés
  différemment des deux côtés, mais rien ne dit où « places manquantes par rôle » (dérivé de
  `class_bounds` et des candidatures `accepted`) est calculé ni si le filtre est un paramètre de requête
  ou un tri client.
- **Concerné** : AD-4, AD-11, Capability Map CAP-4.
- **Correction** : `list_public_events` renvoie par événement les places manquantes par rôle calculées
  côté serveur et accepte `?instance=&role=` ; le « < 2 s » devient une propriété testée sur `sqlite::memory:`.

### C12 — Q7, Q8, Q11 renvoyés « avec leur critère » : la spine ne les nomme pas — **basse**

- **Source** (§12, forge « Renvoyé à la spine ») : Q7 (runner existant), Q8 (données lues sans
  reconstruire, mises à jour après vérification dans le client), Q11 (critère « le moins de pièces à
  sécuriser et maintenir seul », SQLite ou PostgreSQL, un ou deux conteneurs, limitation de débit).
- **Spine** : AD-10, AD-6, AD-1, AD-2 et AD-7 répondent de fait, mais sans citer Q7/Q8/Q11 ni le
  critère : un relecteur ne peut pas vérifier que le critère a été appliqué (et C2 montre qu'il ne l'est
  pas pour Q8). Le `.memlog.md` n'est pas le document de référence.
- **Concerné** : en-tête (`sources`), AD-1, AD-2, AD-6, AD-7, AD-10, Stack.
- **Correction** : une courte section « Questions héritées » : Q7 → AD-10, Q8 → AD-6, Q11 → Stack +
  AD-1/AD-2/AD-7, chacune avec son critère et le verdict en une ligne.

### C13 — « Réseau dédié » et « Caddy unique point d'entrée » : la joignabilité `localhost:PORT` est une hypothèse non marquée — **basse**

- **Source** (§4, §7) : réseau Docker dédié ; Caddy comme unique point d'entrée ; « une stack Docker
  Compose par service » (le Caddy du NAS est donc vraisemblablement lui-même un conteneur).
- **Spine** : AD-8 publie `127.0.0.1:${PORT}:8080` sur un réseau bridge dédié et AD-9 fait
  `reverse_proxy localhost:PORT`. Si le Caddy du NAS tourne en conteneur, `localhost` ne l'atteint pas :
  il faudra un réseau partagé Caddy ↔ application (toujours sans port publié) ou `network_mode: host`
  côté Caddy. Rien n'est marqué `[ASSUMPTION]`.
- **Concerné** : AD-8, AD-9, Structural Seed, Deferred « Question ouverte NAS ».
- **Correction** : vérifier comment tourne le Caddy du NAS, le noter dans la question ouverte NAS, et si
  c'est un conteneur, remplacer le port publié par un réseau externe partagé réservé à ce couple.

## Hors constat (vérifié, bien couvert)

Sécurité du conteneur (non-root, `cap_drop`, `read_only`, limites CPU/mémoire, réseau dédié, jamais
`privileged`/`pid`/socket/bind mount) : AD-8. Secrets hors dépôt et exemples sans hôte : AD-8, AD-9,
conventions. Pas de login/email : AD-3. Données minimales dans l'application, aucun traceur ni script
tiers : AD-7, AD-9, AD-15, AD-17. UTC stocké, fuseau du navigateur : AD-11. fr + en : AD-12. Compétences
de Vincent : la stack reste dans Rust/Axum/SQLx et React/TS/Vite/Tailwind (SQLite plutôt que PostgreSQL,
justifié par AD-2 et le critère Q11). Déploiement par push, branche `main` : AD-10. Hors v1 (PWA,
notifications, staging) : Deferred, avec AD-17 qui verrouille tout appel sortant.
