# Revue « rubric walker » — ARCHITECTURE-SPINE.md

Document jugé : `ARCHITECTURE-SPINE.md` seul (ni memlog ni sources), comme le lira un développeur ou un
agent qui construit un epic sans autre contexte. Grille en neuf points, findings classés par sévérité.

## Verdict

Approuvable après corrections : aucun finding critique, mais cinq findings hauts — deux contradictions
internes (healthcheck vs AD-17, retour arrière vs migrations immuables) et trois trous de décision
(déverrouillage, mutabilité de l'événement par état, droits Docker du runner) — que les epics booking,
salle de jeu et déploiement rencontreront dès leur première story.

## Lecture par point de grille

| # | Point | État |
| --- | --- | --- |
| 1 | Points de divergence des epics v1 | Majoritairement fixés ; manquent le déverrouillage (H3), la mutabilité par état (H4), l'unité « joueur » du solveur (M3), le contenu du DTO public (M7) |
| 2 | Rules applicables, Prevents tenus | Oui pour 15 AD sur 19 ; AD-2 (pool), AD-7 (en-tête absent), AD-10 (retour arrière), AD-19 (périmètre des dépendances) ne se tranchent pas oui/non sur du code |
| 3 | Deferred sans divergence possible | Oui, sauf la forme de `Proposal` (B5) et l'intégration PoW qui heurtera AD-19 (B4) |
| 4 | Cohérence des technos | Cohérente ; une collision AD-8 × AD-17 (H1), jeton HMAC sans état vs « jetons expirés nettoyés » (M2), `dotenvy` hors table (B9) |
| 5 | Dimensions structurelles | Toutes présentes ; exploitation incomplète : `restart` et rotation des logs (M8), mise à jour des dépendances (M9), restauration et E2E (B13) |
| 6 | Paradigme et diagramme vs AD et seed | Cohérents, sauf l'étiquette « hexagonal » qui ne décrit pas `app → infra` concret (M10) |
| 7 | Syntaxe mermaid | Les cinq blocs sont valides (flowchart LR, stateDiagram-v2, flowchart TB avec sous-graphes et cylindre, erDiagram, flowchart LR) ; aucun crochet, parenthèse, accolade, pipe ni guillemet dans un libellé de nœud hors des formes qui les portent ; les libellés erDiagram sont entre guillemets |
| 8 | `[ASSUMPTION]` | Onze marquées ; sept choix produit implicites non marqués (M11) plus un choix de sécurité (H5) |
| 9 | Build substrate | Terse dans l'ensemble ; huit passages de rationale (B10) et quatre redites (B11) |

## Findings

### Critique

Aucun.

### Haute

**H1 — AD-8, AD-17, AD-10 : le healthcheck du binaire contredit l'interdiction de client HTTP.**
AD-8 impose un healthcheck par la sous-commande `healthcheck` du binaire dans une image `scratch` ; AD-17
interdit toute crate client HTTP et fait échouer la CI sur `ureq`, `reqwest` ou un client `hyper-util`. Un
développeur ajoutera `ureq` et cassera la CI, un autre ouvrira la base, un troisième écrira une requête
TCP à la main : trois healthchecks différents.
Correction : préciser dans AD-8 « `healthcheck` = `GET /api/health` HTTP/1.1 écrit à la main sur
`std::net::TcpStream` vers `127.0.0.1:8080`, 200 attendu, aucune crate » et le citer dans AD-17 comme
exception explicite.

**H2 — AD-10 vs AD-2 : « retour arrière = revert + push » est impossible dès qu'un commit porte une migration.**
AD-10 affirme prévenir « un retour arrière impossible » ; AD-2 rend les migrations forward-only et
immuables. Le revert d'un commit qui contient une migration produit un binaire qui ne connaît plus une
migration déjà appliquée : `sqlx::migrate!` refuse de démarrer, le healthcheck est rouge, et rien n'est
prévu.
Correction : écrire « un commit qui ajoute une migration ne se revert pas : retour arrière = migration de
compensation ; le job `deploy` fait un `VACUUM INTO` avant `compose up` quand le push touche
`migrations/` ».

**H3 — AD-4 vs AD-18 : le sort des groupes au déverrouillage n'est pas fixé.**
AD-4 fixe `locked → open` et dit `placement` « défini seulement en `locked` » ; AD-18 veut un `room_name`
« stable au re-verrouillage ». Supprimer les `groups`, `assignments` et `confirmations` au déverrouillage
rend AD-18 impossible ; les conserver contredit la lettre d'AD-4 ; les deux lectures sont ouvertes.
Correction : ajouter à AD-4 « déverrouiller conserve `groups`, `assignments` et `room_name` en l'état,
les masque aux participants et remet `confirmation` à `unasked` ; re-verrouiller repart de cet état » (ou
l'inverse, mais écrit).

**H4 — AD-4, CAP-5 : la mutabilité de l'événement et de la candidature par état n'est pas décidée.**
CAP-5 « modifier son événement » n'est régi que par AD-3 et AD-16 ; AD-4 ne dit pas quels attributs sont
modifiables dans quel état ni ce que devient une candidature quand `region`, `instance`, `faction_scope`
ou un `slot` change (disponibilités orphelines, `group_size` qui change, serveur hors région). Même
silence côté participant : un titulaire placé peut-il changer de classe en `locked` et violer
`class_bounds` ?
Correction : ajouter à AD-4 une ligne « mutabilité » : `instance`, `region`, `faction_scope` immuables
après la première candidature ; `slots` modifiables en `open` seulement (suppression = cascade des
`availabilities`) ; `note`, `discord_url`, `visibility`, `min_cp`, `class_bounds` toujours modifiables ;
contenu de candidature modifiable en `open`, et en `locked` seulement si `unplaced`.

**H5 — AD-10 : les droits Docker du runner sont un choix de sécurité non marqué.**
Le runner est « un processus de l'hôte sous un utilisateur dédié » qui exécute `docker compose up -d
--build`, donc un utilisateur du groupe `docker`, root-équivalent sur le NAS : un workflow déclenché par
GitHub tient les clés de la machine. La phrase « jamais un conteneur avec le socket Docker » donne
l'impression inverse et le choix n'est pas marqué.
Correction : marquer `[ASSUMPTION]` et fixer le mécanisme (« utilisateur du groupe `docker` accepté », ou
`sudo` restreint à `docker compose -f <chemin fixe> up -d --build` et à la copie vers le volume, ou Docker
rootless).

### Moyenne

**M1 — AD-2 : « pool SQLx dimensionné pour sérialiser les écritures » n'est pas décidable sur du code.**
Un pool de taille 1 (lectures sérialisées aussi) et deux pools (un écrivain, N lecteurs) satisfont tous
deux la phrase ; les deux donnent des comportements différents sous charge.
Correction : « deux pools : écriture `max_connections = 1`, lecture `max_connections = 4` ; toute commande
(AD-16) prend le pool d'écriture ».

**M2 — AD-7 vs AD-14 : jeton de formulaire sans état ou stocké ?**
AD-7 définit un jeton HMAC daté, donc vérifiable sans état ; AD-14 fait « nettoyer les jetons expirés »
par la tâche horaire, ce qui suppose une table. Un epic fera un jeton à usage unique en base, l'autre un
HMAC vérifié à la volée.
Correction : trancher « HMAC sans état, rejouable dans sa fenêtre, le limiteur couvre le rejeu » et
retirer la phrase d'AD-14, ou décrire la table `form_tokens` et l'usage unique dans AD-7.

**M3 — AD-13 vs AD-4 : l'unité « joueur » du solveur n'est pas définie.**
AD-4 autorise deux candidatures avec le même `Pseudo[SRV]` ; AD-13 contraint « un joueur dans un seul
groupe » sans dire si le joueur est la candidature ou le `Pseudo[SRV]`. Le solveur et le placement manuel
peuvent placer deux fois la même personne, ou pas, selon l'epic.
Correction : « l'unité du solveur est la candidature ; le doublon de `Pseudo[SRV]` est signalé à
l'organisateur (AD-4) et jamais dédoublonné par `compose()` ».

**M4 — AD-4, AD-11, AD-14, carte RECONF : `expired` et la liste publique, dérivés ou posés par la tâche ?**
AD-4 et AD-11 font calculer `expired` et « créneau passé » côté serveur, mais la carte confie l'état
`expired` à `tasks` et AD-14 place la définition de la liste publique sous la tâche horaire. Dérivé à la
lecture par `Clock` ou posé en base toutes les heures (jusqu'à 59 min de retard) sont deux
implémentations.
Correction : écrire dans AD-11 « `expired`, créneau passé et appartenance à la liste publique sont dérivés
à la lecture depuis `Clock`, jamais stockés ; la tâche horaire ne fait que purger, sauvegarder et
recharger » et corriger la ligne RECONF de la carte.

**M5 — AD-7 : comportement non fixé quand l'en-tête IP manque en production.**
Le repli sur l'adresse du pair n'est permis qu'en `TRUST_CLIENT_IP_HEADER=false`, mais rien ne dit ce
qui se passe en mode confiance quand `CLIENT_IP_HEADER` est absent (Caddy mal configuré, requête LAN). Un
repli silencieux mettrait tout le site derrière la clé du Caddy du NAS.
Correction : « en mode confiance, en-tête absent = 500 journalisé sans IP (fail-closed), jamais l'adresse
du pair ».

**M6 — AD-5, AD-6, AD-19 vs AD-9 : `ETag`/`304` sous `Cache-Control: no-store`.**
L'API est `no-store` (AD-9) et compte sur `ETag` et `304` (AD-5, AD-6, AD-19) ; avec `no-store` le
navigateur n'envoie jamais `If-None-Match` de lui-même et le 304 ne se produit pas. Le cache localStorage
de la référence le fait à la main, les vues organisateur et participant n'ont pas de règle.
Correction : « le client porte `If-None-Match` lui-même (ETag en mémoire pour les vues, en localStorage
pour la référence) ; le serveur ne compte pas sur le cache HTTP du navigateur ».

**M7 — AD-3, AD-5, AD-9 : contenu du DTO public et indexation des événements `unlisted`.**
Rien ne dit si `PublicEvent` (`/e/<id>`, liste publique) expose les candidatures (pseudo, serveur,
classe, CP), et `/e/<id>` n'est pas `noindex` alors qu'un événement `unlisted` est « lien seul » et que
pseudo plus `[SRV]` identifient un joueur. Le périmètre des données personnelles exposées diverge selon
l'epic.
Correction : fixer dans AD-4 le contenu public (par exemple « `PublicEvent` porte les comptes par classe
et par rôle, jamais la liste nominative ») et poser `X-Robots-Tag: noindex` sur `/e/` quand
`visibility = unlisted`.

**M8 — AD-8 : ni politique de redémarrage ni rotation des journaux Docker.**
Le compose durci n'a ni `restart: unless-stopped` ni limite du pilote de journaux ; un crash ou un
redémarrage du NAS laisse le service à terre jusqu'au prochain push, et `tracing` sur stdout remplit le
disque sans borne.
Correction : ajouter `restart: unless-stopped` et `logging: { driver: json-file, options: { max-size:
10m, max-file: "3" } }` à la liste d'AD-8 et aux clés exigées par `check-hardening.sh`.

**M9 — Enveloppe opérationnelle : mise à jour des dépendances et audit de vulnérabilités silencieux.**
Ni décidé ni différé : `cargo audit`/`cargo deny`, `npm audit`, Dependabot ou équivalent, pour un service
public dont l'image `scratch` ne reçoit aucun correctif hors le binaire. Un epic déploiement peut livrer
sans jamais revoir une CVE.
Correction : ajouter à AD-10 « `cargo deny check advisories` et `npm audit --audit-level=high` dans le job
`ci` ; Dependabot hebdomadaire sur Cargo et npm », ou l'inscrire en Deferred avec une échéance.

**M10 — Design Paradigm : « hexagonal allégé » ne décrit pas les dépendances déclarées.**
Hexagonal suggère des ports (traits) implémentés par `infra`, mais le diagramme et la table font dépendre
`app` d'`infra` concrètement et seule l'horloge est dite injectée (AD-11). Un développeur fera des traits
de dépôt mockés, un autre des appels directs.
Correction : écrire « couches concrètes : `app` appelle `infra::sqlite` directement ; seuls `Clock` et le
générateur d'ids/secrets sont des traits injectés, pour les tests » et renommer le paradigme « monolithe
modulaire en couches, noyau pur ».

**M11 — Hypothèses implicites non marquées (point 8).**
Choix produit faits sans Vincent et présentés comme acquis : `discord_url` réservé à l'organisateur et au
participant `accepted` (AD-4) ; doublons de `Pseudo[SRV]` permis (AD-4) ; `expired` sans effet
automatique sur le placement (AD-4) ; `locked → open` seulement s'il reste un créneau futur (AD-4) ; note
≤ 280 caractères sans URL (AD-7) ; `min_cp` hors contraintes du solveur (AD-4, AD-13) ; `confirm_by`
choisi par l'organisateur à chaque demande (AD-4).
Correction : marquer chacun `[ASSUMPTION]` ou citer la section du forge qui le verrouille.

### Basse

**B1 — AD-3 : « quatre secrets » pour trois préfixes secrets.**
`/o`, `/j`, `/p` portent un secret de 256 bits, `/e` un id de 128 bits ; la cardinalité (un `/o` et un
`/j` par événement, un `/p` par candidature) n'est qu'implicite.
Correction : « trois secrets de lien : `/o` et `/j` par événement, `/p` par candidature ».

**B2 — AD-3, titre : « aucun secret en clair côté serveur » est faux pour les secrets d'environnement.**
`ADMIN_SECRET` et `FORM_TOKEN_SECRET` sont lus en clair ; seul le secret de lien est haché.
Correction : « aucun secret de lien en clair en base ».

**B3 — AD-5 : la liste des DTO est incomplète.**
« Un DTO par droit » n'en nomme que trois ; la liste publique, la référence, l'admin, la réponse de
création (liens `/o`, `/j`), celle de candidature (lien `/p`) et la `Proposal` n'ont pas de nom.
Correction : compléter la liste ou écrire « au moins » et fixer la règle de nommage `<Droit><Objet>`.

**B4 — AD-19 : périmètre de « dépendances front » et PoW différée.**
La règle ne dit pas si les devDependencies (Vite, TypeScript, vitest, eslint, pourtant dans la Stack)
sont incluses ; et la preuve de travail différée exigera un solveur PoW côté client, donc une dépendance
ou du code maison.
Correction : « dépendances d'exécution du bundle limitées à… ; devDependencies libres sous revue » et
noter dans Deferred « PoW UI = solveur TS maison, sans dépendance ».

**B5 — AD-13, Deferred : la forme de `Proposal` n'est pas fixée.**
Banc inclus ou non, liste des non placés, un groupe par créneau retenu : `apply_placement` doit l'accepter
telle quelle, à l'identique du placement manuel.
Correction : « `Proposal` = la structure d'entrée d'`apply_placement` : pour chaque `group`, titulaires
`(party, index)` et banc ordonné ; les candidatures absentes sont `unplaced` ».

**B6 — AD-10 : deux mécanismes sans nom.**
« Copie de `reference/` dans le volume » et « garde-fou d'immuabilité des migrations » n'ont pas de
mécanisme ; la copie ne doit pas passer par un bind mount (AD-8).
Correction : « copie par `docker compose cp reference/. server:/data/reference/` ; garde-fou = `git diff
--name-status <sha-déployé>..HEAD -- crates/server/migrations/` sans ligne `M` ni `D` ».

**B7 — AD-17 : le test `cargo tree` trouvera toujours `hyper-util`.**
`hyper-util` est une dépendance d'axum côté serveur ; un `cargo tree` nu déclenche l'échec en permanence.
Correction : « test sur `cargo tree -e features` cherchant `hyper-util feature "client"` ».

**B8 — AD-6 vs AD-12 : `fr` optionnel ou libellés bilingues ?**
AD-6 rend `fr` optionnel avec repli sur `en`, AD-12 dit « libellés de référence bilingues ».
Correction : aligner AD-12 sur AD-6 (« `en` obligatoire, `fr` avec repli »).

**B9 — Stack et AD-9 : `dotenvy` absent de la table, `X-Forwarded-Proto` posé mais lu par personne.**
`dotenvy` apparaît dans la convention Config ; `PUBLIC_BASE_URL` est la seule source d'URL absolue, donc
l'en-tête du VPS ne sert à rien d'écrit.
Correction : ajouter `dotenvy` à la Stack et retirer `X-Forwarded-Proto` d'AD-9 ou dire qui le lit.

**B10 — Rationale à déplacer vers le memlog (point 9).**
AD-9 « Déviation assumée du forge … l'edge garde les en-têtes de transport » ; AD-3 « indevinable, parce
qu'un événement peut être lien seul », « (les déplieurs de liens Discord font des GET) », « le risque du
PC partagé est celui que le forge a accepté » ; AD-5 « (rien à régler dans les deux Caddy, aucune
connexion longue) » ; AD-7 « (plusieurs candidatures par jour … sont légitimes) », « la rétention 24 h
max est satisfaite de fait » ; AD-8 « scratch plutôt qu'alpine : aucun shell … » ; AD-1 « (le binaire ne
peut pas être livré avec des assets périmés) » ; AD-13 « la v1 est complète sans `compose()`, construit
en dernier » (séquencement, pas architecture).
Correction : ne garder que la règle et une trace `(forge : déviation)` ; le pourquoi reste dans
`.memlog.md`.

**B11 — Redites (point 9).**
Les lignes Ids & secrets, Mutation, Journalisation et Dépôt public des Conventions répètent AD-3, AD-16,
AD-15 et AD-10 ; « sans rebuild » apparaît dans AD-6 et deux fois dans AD-10 ; AD-4 tient en un seul
paragraphe de 25 lignes où un relecteur ne peut pas pointer une règle.
Correction : réduire chaque ligne redondante à un renvoi « voir AD-n » et découper AD-4 en puces
attributs / invariants / états / candidature / propriétaires / visibilité.

**B12 — CONTACT et CAP-n : définis seulement par la carte.**
« Quatre gestes sans canal » et les libellés CAP-1 à CAP-7 n'existent que dans la carte ; un lecteur sans
le forge ne sait pas quels sont les gestes.
Correction : nommer les quatre gestes dans la ligne CONTACT ou renvoyer à la section du forge qui les
fixe.

**B13 — Tests et sauvegarde : E2E navigateur et restauration ni décidés ni différés.**
La ligne Tests s'arrête à `vitest` unitaire ; la restauration d'une sauvegarde `VACUUM INTO` (arrêt,
remplacement du fichier, fichiers `-wal`/`-shm`) n'est pas écrite.
Correction : ajouter en Deferred « E2E navigateur : hors v1 » et dans AD-2 « restauration = `docker
compose stop`, copie du `.db` de sauvegarde sur `/data/app.db`, suppression des `-wal`/`-shm`, `up` ».

## Décompte

29 findings : 0 critique, 5 hautes, 11 moyennes, 13 basses.
