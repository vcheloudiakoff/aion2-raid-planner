# Revue adversaire : deux unités conformes, une construction incompatible

> Revue du 2026-11-05. Cible : `ARCHITECTURE-SPINE.md` (draft du même jour, après réconciliation des trois sources).
> Lens : pour chaque zone de contact, construire deux unités un niveau en dessous (deux stories de la v1, par deux
> développeurs ou deux agents qui ne se parlent pas), chacune obéissant à chaque AD à la lettre, et montrer où elles
> se heurtent. Chaque paire est un trou à fermer par un AD resserré. Les epics cités sont ceux pressentis :
> (1) création et liens, (2) candidature et vue participant, (3) vue organisateur, tri, verrouillage, salle,
> (4) composition, (5) re-confirmation et contact, (6) liste publique, (7) anti-abus et administration,
> (8) déploiement et isolation, (9) i18n et front mobile.

## Verdict

La spine tient bien ses frontières (couches, agrégat unique, commandes transactionnelles, droits par lien) mais
laisse ouverts les points de contact que deux stories indépendantes codent chacune à leur façon (forme des
placements, retrait d'un joueur placé, état `expired`, secret hors de sa route, health-check derrière l'en-tête IP,
copie de la référence) : quinze paires, deux critiques et cinq hautes, toutes fermables en resserrant des AD
existants, sans AD nouveau ni changement de produit.

## Paires

### P1 : deux formes de `assignments` et deux sémantiques d'`apply_placement` (critique)

- **Unité A : epic 3, story « verrouiller l'événement ».** Lit AD-4 : « chacun avec `group_size` titulaires selon la
  disposition de l'instance (`parties × party_size`, un `assignment` par `(party, index)`) ». `lock_event` crée donc,
  par groupe, `parties × party_size` lignes `assignments` vides (`candidature_id NULL`), index unique
  `(group_id, party, index)` ; le banc n'a pas de ligne tant qu'il est vide. `apply_placement` déplace un joueur :
  `UPDATE assignments SET candidature_id = ? WHERE group_id = ? AND party = ? AND index = ?`. `placement` est une
  colonne dénormalisée de `candidatures` (AD-4 le décrit comme un attribut de la candidature) tenue à jour par la
  même commande.
- **Unité B : epic 4, story « appliquer une proposition du solveur ».** Lit le diagramme ER :
  `ASSIGNMENT }o--|| CANDIDATURE` (candidature obligatoire) et « titulaire (party, index) ou banc ». Une ligne
  `assignments` n'existe que pour une candidature placée : `(group_id, candidature_id, party | NULL, index | NULL,
  bench_index | NULL)`, unique sur `candidature_id` (« un joueur dans un seul groupe », AD-13). `apply_placement`
  reçoit la `Proposal` entière, placement complet de l'événement, et remplace tout (`DELETE` puis `INSERT`) dans la
  transaction AD-16. `placement` n'est pas stocké, il est dérivé des `assignments` dans les requêtes.
- **Collision.** Même table, deux schémas (lignes pré-créées nullables contre lignes à la demande), deux clés
  d'unicité, deux payloads sous le même nom de commande (delta d'une case contre remplacement complet). La migration
  de l'une viole le `NOT NULL` de l'autre. `organizer_view` (A) lit `candidatures.placement`, `participant_view` (B)
  dérive des `assignments` : après une manipulation manuelle, les deux vues divergent. Enfin, placer le même joueur
  dans deux groupes de créneaux différents (A le permet : rien ne l'interdit hors AD-13, qui ne lie que le solveur)
  est irreprésentable dans `placement ∈ {…}` (une seule valeur) et rejeté par B.
- **Correction (AD-4).** `lock_event` crée les `groups` seulement ; `assignments` n'a de ligne que pour une
  candidature placée et `apply_placement` en est l'unique writer ; `placement` n'est jamais stocké sur la
  candidature, il est dérivé ; invariants portés par des index : `UNIQUE(candidature_id)` (au plus un placement par
  candidature, tous groupes et créneaux confondus), `UNIQUE(group_id, party, index)`, `UNIQUE(group_id, bench_index)`.
  `apply_placement` reçoit le placement complet de l'événement (liste `(candidatureId, groupId, party, index |
  benchIndex)`) et remplace tout : un déplacement manuel renvoie le placement entier, cohérent avec « dernière écriture
  gagne » (AD-16).

### P2 : retrait d'un titulaire placé, deux propriétaires et aucune transition écrite (critique)

- **Unité A : epic 5 (ou 2), story « retirer ma participation ».** AD-4 : « le participant possède … son retrait »,
  « l'organisateur possède `status` et `placement` ». `withdraw` vérifie le secret `/p`, pose `status = withdrawn` et
  ne touche pas `assignments` (propriété de l'organisateur). L'invariant « `withdrawn` n'est jamais placée » est
  laissé à l'organisateur.
- **Unité B : epic 3, story « vue organisateur et grille ».** Se fie à l'invariant : `organizer_view` affiche la
  composition depuis `assignments` ; `apply_placement` ne revalide `status = accepted` que pour les lignes du payload ;
  le re-verrouillage et la recette de salle (AD-18) listent les titulaires depuis `assignments`.
- **Collision.** Un titulaire se retire la veille : A écrit `withdrawn`, la ligne `assignments` reste ; B l'affiche
  toujours titulaire et dans la recette, la vue participant dit « retiré ». Variante : l'organisateur « possède
  `status` », B lui permet `withdrawn → accepted` et le replace, le participant n'a plus de geste. Autre variante : A
  laisse le participant changer de classe après acceptation sans toucher `status` ; B remet `pending` (« l'organisateur
  a accepté un Clerc, pas un Gladiateur ») et le joueur reste placé en `pending`. L'origine du lien (`/j` ou `/e`) n'est
  pas stockée : un retour ne sait pas s'il redevient `accepted` ou `pending`. Une candidature arrivant par `/j` après
  `locked` est acceptée et sans placement (B : banc tardif) ou refusée 409 (A).
- **Correction (AD-4).** Machine d'états de la candidature avec un propriétaire par transition : participant
  `pending|accepted → withdrawn` (terminal, revenir = nouvelle candidature) ; organisateur `pending → accepted|rejected`,
  `accepted ↔ rejected`, jamais depuis `withdrawn`. `withdraw` et `set_status(rejected)` suppriment la ligne
  `assignments` dans la même transaction (exception nommée à la propriété du placement, signalée dans le DTO
  organisateur). `origin ∈ {direct, public}` est stocké ; une modification de contenu ne change jamais `status` ; en
  `locked`, le participant ne peut que répondre ou se retirer (`event.locked` sinon) et une candidature nouvelle est
  acceptée selon son lien mais `unplaced`.

### P3 : `expired`, état dérivé à la lecture ou état écrit par la tâche horaire (haute)

- **Unité A : epic 5, story « répondre à une demande de confirmation ».** AD-4 « `expired` calculé par le serveur
  quand l'échéance passe », AD-11 « échéance de confirmation … calculée côté serveur », horloge injectée : `expired`
  n'est pas stocké ; `confirmation_state(row, now)` le dérive dans `participant_view`, et `answer_confirmation` après
  `confirm_by` renvoie `confirmation.deadline_passed`.
- **Unité B : epic 7 ou 8, story « tâches périodiques ».** Carte des capacités : « RECONF … `tasks` (état
  `expired`) », AD-14 « une tâche tokio horaire ». Elle écrit, par une commande `expire_confirmations` (AD-16),
  `state = 'expired'` sur les `asked` dépassées, à chaque heure pleine. `answer_confirmation` ne regarde que la
  colonne : entre l'échéance et le tick suivant (jusqu'à 59 minutes), une réponse tardive est acceptée.
  `organizer_view` lit la colonne.
- **Collision.** Pendant l'heure qui suit l'échéance, la vue participant (A) dit « expiré » et refuse la réponse, la
  vue organisateur (B) dit « en attente » ; une réponse acceptée par B à H + 40 min est `confirmed` chez B et
  affichée `expired` chez A si A dérive depuis `confirm_by` sans regarder `answered_at`. Deuxième heurt : A pose la
  demande par candidature (`POST /api/candidatures/{id}/confirmation`), B pour tout l'événement
  (`POST /api/events/{id}/confirmations { confirmBy }` pour tous les titulaires) ; l'ER dit 0..1 confirmation par
  candidature mais rien sur la ré-émission (nouvelle ligne et historique, ou écrasement).
- **Correction (AD-4, AD-14).** `expired` n'est jamais écrit : une seule fonction
  `domain::confirmation_state(asked_at, confirm_by, answer, now)` sert à toutes les requêtes et à
  `answer_confirmation` (réponse après `confirm_by` = `confirmation.deadline_passed`) ; la tâche horaire ne touche pas
  aux confirmations. Une ligne par candidature, écrasée par une nouvelle demande : `request_confirmation` prend une
  liste de candidatures et une échéance et remet chacune à `asked`.

### P4 : un secret au mauvais endroit, l'extracteur, la route et le client ne s'entendent pas sur « deux secrets » (haute)

- **Unité A : epic 2, story « candidater depuis `/e` et `/j` ».** Une seule route
  `POST /api/events/{id}/candidatures`, `X-Link-Secret` optionnel : absent = `pending` ; présent = doit être le hash
  `/j`, sinon 403. Le client `fetch` envoie le secret lu dans le chemin courant (`/j/<secret>`), rien sur `/e/<id>`.
- **Unité B : epic 3, story « écran organisateur ».** « Mes événements » en `localStorage` (AD-3) : pour offrir « voir
  la page publique en tant qu'organisateur » et « m'ajouter moi-même », le client lit le secret `/o` en `localStorage`
  et l'attache à toute requête concernant cet `eventId`, quel que soit le chemin. Côté serveur, l'extracteur résout
  tout secret en `Right::{Organizer, Join, Participant}` en cherchant le hash dans trois tables, et
  `submit_candidature` accepte `Organizer` comme équivalent de `Join`.
- **Collision.** Sur `/e/<id>`, le client B envoie le secret `/o` ; le handler A reçoit un secret qui n'est pas le hash
  `/j` : 403 pour l'organisateur sur sa propre page. Inversement, le handler B accepte un secret `/o` sur la route de
  candidature, geste qui est un 403 chez A ; `types.ts` et les codes (`auth.forbidden` contre `auth.wrong_right`)
  divergent. Un `/p` d'un autre événement présenté sur `/api/events/{id}` : A répond 403, B répond 404 ; aucune règle.
  L'extracteur B cherche dans trois tables à chaque requête, A dans la seule table de la route : deux implémentations,
  deux temps de réponse, deux surfaces d'énumération.
- **Correction (AD-3).** Un extracteur unique `LinkSecret` qui ne résout qu'un droit, celui que la route déclare
  accepter, et vérifie que l'objet visé appartient à la portée du secret ; secret d'un autre droit ou d'une autre portée
  = 403 `auth.wrong_right`, secret inconnu = 404, aucun secret sur une route sous droit = 401 `auth.missing`. Le client
  n'envoie que le secret du chemin courant, jamais un secret lu en `localStorage` (« Mes événements » ne sert qu'à
  naviguer vers `/o/…`) ; l'organisateur qui veut jouer passe par son propre `/j`.

### P5 : `/api/health`, l'en-tête IP obligatoire et la sous-commande `healthcheck` (haute)

- **Unité A : epic 7, story « limiteur par IP ».** AD-7 : en production `TRUST_CLIENT_IP_HEADER=true`, la clé est
  l'en-tête, « repli sur l'adresse du pair uniquement si … `false` ». Une requête sans en-tête n'a donc pas de clé :
  fail-closed, 400 `limit.client_ip_missing`. Toutes les routes `/api/*` passent le middleware, « lectures »
  comprises, `/api/health` est une lecture. Variante : `CLIENT_IP_HEADER=X-Forwarded-For` (le VPS le pose aussi,
  memlog), valeur multi-adresses, A prend la première, celle que le client contrôle si le VPS ajoute au lieu d'écraser.
- **Unité B : epic 8, story « déploiement et healthcheck ».** AD-10 : le runner fait `GET /api/health` dix fois sur
  `127.0.0.1:PORT`, sans passer par le Caddy du NAS, donc sans `X-Client-IP`. AD-8 :
  `HEALTHCHECK CMD ["/server", "healthcheck"]` ; sans client HTTP (AD-17), B l'implémente par ouverture de
  `/data/app.db` et `SELECT 1`. `/api/health` répond 200 si migrations appliquées et référence chargée.
- **Collision.** Le premier déploiement est rouge : les dix essais du runner reçoivent 400 (en-tête absent) ou 429
  (dix essais plus le healthcheck Docker, comptés dans la classe « lectures » sous une clé vide commune). Si B
  contourne avec `TRUST_CLIENT_IP_HEADER=false` en production, tout le limiteur retombe sur l'adresse du pair, le
  Caddy du NAS, une seule clé pour toute la planète. Par ailleurs la sous-commande de B ne teste pas le serveur HTTP
  (processus bloqué, `SELECT 1` passe) et `/api/health` ne vérifie rien de ce que la sous-commande vérifie : deux
  « santés » différentes pour deux consommateurs.
- **Correction (AD-7, AD-8, AD-10).** Classes de route nommées, dont une classe « sans clé » (`/api/health`, assets,
  `index.html`) exemptée du limiteur et de l'extracteur d'IP ; sur une route à clé en production, en-tête absent ou
  multi-valué = 400 `limit.client_ip_missing` ; `CLIENT_IP_HEADER` est mono-valeur (`X-Client-IP` par défaut), jamais
  `X-Forwarded-For`. `/api/health` et la sous-commande `healthcheck` appellent la même fonction `app::health()` (base
  ouverte, migrations à jour, référence chargée) ; le runner teste HTTP, le conteneur teste le processus.

### P6 : copier `reference/` dans le volume, le runner, l'utilisateur 10001 et le rechargement par mtime (haute)

- **Unité A : epic 8, story « push sur `reference/` sans rebuild ».** AD-10 : « copie de `reference/` dans le
  volume ». Pas de socket Docker, pas de bind mount en compose, pas de `docker exec` possible (`scratch`, aucun
  shell) : A fait `docker compose cp reference/. server:/data/reference/` depuis le runner (fichier par fichier,
  propriétaire root:root, mode du dépôt), puis `curl /api/health`. Au premier déploiement, `up` d'abord (le conteneur
  doit tourner pour `cp`).
- **Unité B : CAP-6, story « chargeur et rechargement de la référence ».** AD-6 : « recharge quand un mtime change
  (scrutation ≤ 60 s), après validation complète : fichier invalide = ancien jeu conservé et erreur journalisée ». B
  mémorise le mtime du dossier vu au dernier essai, puis valide ; en cas d'échec il garde l'ancien jeu et attend le
  prochain changement de mtime. « L'image embarque une copie, utilisée seulement si `/data/reference/` est vide » : B
  copie ce jeu embarqué dans `/data/reference/` au premier démarrage pour n'avoir qu'un chemin de lecture.
- **Collision.** La copie de A n'est pas atomique : B observe le mtime entre `servers.toml` (nouveau, qui référence une
  région nouvelle) et `regions.toml` (ancien), la validation échoue, B retient ce mtime, et la dernière écriture de A
  ne change plus le mtime du dossier (même seconde) : B reste sur l'ancien jeu jusqu'au prochain redémarrage, le job
  est vert (health OK) et Vincent croit le serveur ajouté. Deuxième heurt : la copie de B au premier démarrage crée
  `/data/reference/` en 10001:10001, `docker cp` de A écrit ensuite root:root (lisible), mais un fichier retiré du
  dépôt n'est jamais retiré du volume (AD-6 dit « jamais par suppression » pour les entrées, rien pour les fichiers) ;
  un `foo.toml` fantôme reste chargé. Troisième : si A choisit plutôt
  `docker run --rm -v vol:/data -v $PWD/reference:/src:ro alpine cp …`, c'est un bind mount d'un répertoire du NAS
  hors compose, invisible à `check-hardening.sh`.
- **Correction (AD-6, AD-10).** La copie est atomique : le runner écrit `reference.next/` puis renomme (ou un seul
  fichier `reference.toml` concaténé, ou un manifeste écrit en dernier) ; le chargeur ne mémorise un mtime qu'après un
  chargement réussi et réessaie à chaque tick sinon ; il ne lit que les cinq fichiers nommés par la spine. Le
  mécanisme est fixé (`docker compose cp` vers le conteneur en marche, jamais un conteneur auxiliaire avec bind mount ;
  `check-hardening.sh` grep aussi `.github/workflows/`) et le serveur n'écrit jamais dans `/data/reference/` : le jeu
  embarqué est lu en mémoire.

### P7 : la référence pendant une commande, instantané, `enabled = false` et disposition d'un groupe verrouillé (haute)

- **Unité A : epic 3, story « verrouiller ».** `lock_event` lit `reference.current()` dans la transaction pour
  `parties × party_size` et `room_label` ; il ne les stocke pas sur le groupe (« jamais en dur ») et `organizer_view`
  recalcule la disposition depuis l'instance à chaque lecture. Par AD-16 « applique les règles de `domain` », il
  revalide l'agrégat entier, donc chaque candidature contre la référence courante.
- **Unité B : epic 2, story « modifier ma candidature ».** AD-6 : « validation … aux écritures qui choisissent une
  valeur (… changement de classe) ». Le formulaire renvoie tous les champs, B valide tout le payload : `class` doit
  exister et être `enabled`.
- **Collision.** Vincent passe `enabled = false` sur une classe, ou l'éditeur fait passer une instance de 5 à 6 par
  équipe. Chez A, `lock_event` échoue `candidature.class_unknown` à cause de la candidature d'un tiers, l'organisateur
  est bloqué ; après rechargement, un groupe verrouillé à 10 affiche 12 cases chez A et 10 placements dans la vue
  participant qui compte les `assignments`. Chez B, un participant qui ne change que ses disponibilités reçoit
  `reference.class_disabled`. Une `Proposal` calculée sur l'instantané d'avant rechargement (AD-13, `spawn_blocking`)
  est appliquée par une commande qui relit après : 10 cases contre 12, résultat selon l'ordre des vérifications.
- **Correction (AD-6, AD-4, AD-18).** Le jeu de référence est un `Arc<ReferenceSet>` immuable pris une fois par
  requête ou commande et passé au `domain` ; la règle « existe et est `enabled` » ne porte que sur les champs que
  l'écriture pose (un champ renvoyé à l'identique n'est pas « choisi ») ; aucune écriture ne revalide le contenu des
  autres candidatures ; `GET /api/reference` renvoie les entrées désactivées avec `enabled: false` (les sélecteurs les
  cachent, les libellés se résolvent). Le groupe fige à son verrouillage `parties`, `party_size` et `room_label` ;
  aucune lecture d'un groupe ne consulte la référence pour sa disposition.

### P8 : jeton de formulaire, HMAC sans état ou jeton stocké à usage unique (moyenne)

- **Unité A : epic 7, story « jeton de formulaire ».** AD-7 : HMAC, âge entre 3 s et 1 h. A fait sans état :
  `base64url(ts ‖ nonce ‖ HMAC(ts ‖ nonce))`, vérification pure, rejouable pendant une heure (le limiteur borne le
  rejeu) ; `GET /api/form-token` n'écrit rien (AD-3 : GET sans effet).
- **Unité B : epic 8 ou CAP-7, story « tâche horaire ».** AD-14 : « la même tâche nettoie les jetons expirés », donc il
  existe une table `form_tokens` ; B la crée, `GET /api/form-token` y insère une ligne, le POST la consomme (usage
  unique), la tâche purge.
- **Collision.** Le client de l'epic 2 (AD-19 : « jeton demandé en parallèle à l'ouverture de l'écran », un seul
  aller-retour) demande un jeton, soumet, reçoit `candidature.cp_out_of_range`, corrige, resoumet le même jeton : chez
  B `form.token_used`, chez A OK. Deux stories, deux comportements du même écran, et un GET qui écrit en base chez B.
  Deuxième heurt : qui compte pour `MAX_CANDIDATURES_PER_HOUR` ? A au middleware (les tentatives, 4xx compris), une
  autre story dans la commande (les lignes commises) : un bot qui échoue à la validation déclenche le 503 chez A et
  jamais ailleurs.
- **Correction (AD-7, AD-14).** Le jeton est sans état et sans usage unique (HMAC sur `ts ‖ nonce ‖ classe de
  route`), aucune table, rien à nettoyer ; AD-14 retire « jetons expirés ». Les quotas par IP se comptent au middleware
  sur les requêtes, le disjoncteur global se compte dans la commande sur les créations commises, et 429 comme 503
  portent un `Retry-After` calculé par un seul module `http::limits`.

### P9 : déverrouiller puis re-verrouiller, que deviennent groupes, placements et noms de salle (moyenne)

- **Unité A : epic 3, story « déverrouiller ».** AD-4 : groupes « après verrouillage, 0..K » ; `locked → open` : A
  supprime groupes et `assignments` (l'événement est de nouveau ouvert), un nouveau `lock_event` recrée tout avec de
  nouveaux codes Crockford.
- **Unité B : epic 3 ou ROOM, story « nom de salle ».** AD-18 : « stable au re-verrouillage sauf demande explicite » :
  B conserve `groups` et `assignments` pendant `open`, les cache des DTO participants, et `lock_event` les réapparie
  par `slot_id`.
- **Collision.** A perd les noms et les placements au moindre déverrouillage, alors que l'organisateur a déjà posté
  la salle sur Discord. B garde des groupes dans un événement `open` que « modifier mon événement » (epic 1 ou 5)
  peut casser : supprimer un créneau référencé par un groupe donne, avec `foreign_keys = ON`, un 500 chez l'un ou une
  cascade silencieuse chez l'autre ; re-verrouiller avec le même créneau deux fois (« doublon permis ») : B apparie par
  `slot_id` et ne sait pas lequel des deux groupes garde quel nom.
- **Correction (AD-4, AD-18).** Le déverrouillage ne supprime rien : groupes et placements restent, invisibles aux
  participants tant que `open` ; `lock_event` réapparie par `(slot_id, rang d'occurrence)` et ne génère un nom que
  pour un groupe nouveau ou sur `regenerate_room_name` explicite ; supprimer un créneau en `open` supprime ses groupes
  par cascade déclarée (`ON DELETE CASCADE`), le DTO organisateur prévient (`slot.hasGroups`), et c'est la seule façon
  de perdre un nom de salle.

### P10 : horodatages, `+00:00` de sqlx, `Z` de serde, `datetime('now')` de SQLite (moyenne)

- **Unité A : epic 1, story « créer l'événement ».** sqlx avec la feature `chrono` : `DateTime<Utc>` lié tel quel ;
  sqlx l'encode en texte RFC 3339 avec suffixe `+00:00` (comportement des lignes 0.7 et 0.8, à vérifier en 0.9) ;
  dans le DTO, serde émet `Z`. Conforme à AD-11 des deux côtés.
- **Unité B : epic 7 ou 8, story « purge ».** AD-11 : « TEXT en SQLite » ; B compare en SQL, `WHERE ends_at < ?` avec
  `now.format("%Y-%m-%dT%H:%M:%SZ")`, ou, par commodité, `datetime('now', '-7 days')` : l'horloge injectée n'est plus
  consultée, les tests avec `Clock` fixe passent contre l'heure réelle.
- **Collision.** Comparaison lexicographique entre `…T21:00:00+00:00` (A) et `…T21:00:00Z` (B) : égalité fausse,
  ordre faux à la seconde près, index inutile ; une colonne lue comme `String` par un DTO renvoie `+00:00` dans un JSON
  qui promet `Z`, `types.ts` ne voit rien. Et « créneau futur » (AD-4), « non commencé » (AD-14) et « fin du dernier
  créneau » (purge) sont codés trois fois, par trois unités, avec trois comparaisons.
- **Correction (AD-11).** Un newtype `domain::Timestamp` avec une seule forme textuelle `YYYY-MM-DDTHH:MM:SSZ`
  (seconde, sans fraction) pour SQLite et JSON, implémentant `sqlx::Type` et `serde` ; interdits : `String` pour un
  instant, `datetime('now')` ou `CURRENT_TIMESTAMP` en SQL. Trois prédicats de `domain` (`slot.is_upcoming(now)`,
  `slot.is_over(now)`, `event.last_slot_end()`) servent au déverrouillage, à la liste publique et à la purge.

### P11 : `visibility`, l'organisateur et l'administration écrivent la même colonne (moyenne)

- **Unité A : epic 7, story « retirer de la liste ».** `admin::unlist_event` pose `visibility = unlisted` par AD-16.
- **Unité B : epic 1 ou 5, story « modifier mon événement ».** `visibility` est un attribut de l'événement (AD-4), le
  formulaire d'édition le renvoie tel que l'organisateur l'a laissé : `listed`.
- **Collision.** L'organisateur ré-enregistre son événement une minute après le retrait administratif et le voilà
  relisté ; il ne sait pas pourquoi son événement avait disparu, l'administration ne voit pas qu'il est revenu.
  Variante à trois writers si une story matérialise « plus de créneau à venir » en écrivant `unlisted` depuis la
  tâche horaire.
- **Correction (AD-4, ADMIN).** `visibility` appartient à l'organisateur seul ; l'administration pose un drapeau
  distinct `admin_unlisted_at` qu'elle seule écrit ; liste publique = `visibility = listed AND admin_unlisted_at IS
  NULL AND un créneau à venir`, calculée à la lecture ; le DTO organisateur expose `adminUnlisted: true` en lecture
  seule.

### P12 : codes d'erreur et clés `fields`, deux taxonomies pour un seul `t()` (moyenne)

- **Unité A : epic 2.** `candidature.cp_out_of_range`, `fields: { "cp": "candidature.cp_out_of_range" }`, clés =
  noms JSON camelCase (`availabilities[2].slotId`), 400.
- **Unité B : epic 1.** `validation.cp_out_of_range` pour `min_cp`, `fields: { "min_cp": … }` (noms Rust),
  `reference.unknown_server` là où A dit `candidature.server_unknown`, 422 pour la validation.
- **Collision.** `fr.json` plat et le test « mêmes clés des deux côtés » ne garantissent pas qu'un code serveur a une
  traduction : le participant voit `validation.cp_out_of_range` brut ; le composant de formulaire qui surligne `fields`
  cherche `minCp` et trouve `min_cp` ; deux statuts HTTP pour la même faute, deux branches dans le client.
- **Correction (AD-5).** `code = <entité>.<cas>` avec entité dans une liste fermée (`event, slot, candidature,
  placement, confirmation, reference, auth, form, limit, admin`) et un registre Rust unique (`http::errors::Code`, enum
  exhaustif) ; `fields` a pour clés les chemins JSON camelCase (`availabilities.2.slotId`) ; validation = 400, conflit
  d'état = 409, droit = 401/403/404. Un test de CI exporte l'enum (sous-commande `list-error-codes`) et échoue si
  `errors.<code>` manque dans une locale.

### P13 : `ETag` sous `no-store`, un seul wrapper `fetch`, et la SPA périmée après déploiement (moyenne)

- **Unité A : epic 3, story « rafraîchissement de la vue organisateur ».** AD-5 : ETag et 304 toutes les 10 s. A
  compte sur le cache du navigateur pour le conditionnel ; `client.get()` fait `res.json()` sans condition.
- **Unité B : epic 5, story « vue participant ».** AD-9 impose `Cache-Control: no-store` sur l'API : le navigateur
  n'envoie jamais `If-None-Match` de lui-même ; B gère l'ETag à la main dans le même `client.ts` et renvoie le corps
  mémorisé sur 304.
- **Collision.** Un seul module `web/src/api/client.ts`, deux contrats : avec A, jamais de 304 (toutes les 10 s, tous
  les écrans, corps complet) ; avec B, le `res.json()` de A explose sur un 304 sans corps. Deuxième heurt : après un
  déploiement qui ajoute un champ requis au payload d'`apply_placement`, l'organisateur qui a l'écran ouvert depuis la
  veille poste l'ancien payload avec le `types.ts` périmé de son onglet : 400, et rien ne lui dit de recharger ;
  « rupture = `/api/v2` » ne dit pas qui décide qu'un champ requis ajouté est une rupture.
- **Correction (AD-5, AD-9).** Le conditionnel est manuel : `client.ts` garde `{ etag, body }` par URL en mémoire,
  envoie `If-None-Match`, traduit 304 en corps mémorisé ; l'API reste `no-store`. Chaque réponse `/api/*` porte
  `X-App-Build: <hash de build>` et le client recharge la page quand il change (sur un GET, jamais en plein POST) ;
  ajouter un champ requis à un payload ou retirer un champ d'un DTO est une rupture par définition.

### P14 : `localStorage`, deux `/p` pour un même événement et trois formes de la même clé (basse)

- **Unité A : epic 2.** `links[eventId] = { p: secret }` ; un joueur qui inscrit deux personnages (« deux candidatures
  avec le même `Pseudo[SRV]` sont permises ») écrase le premier lien, perdu sans récupération (AD-3).
- **Unité B : epic 1.** `links[eventId] = { organizer, join }` pour « Mes événements » et « Dupliquer » ; l'epic 6 ou
  9 lit une troisième forme pour la liste.
- **Collision.** Même clé, trois schémas ; l'organisateur qui candidate perd `organizer` ou `p` selon l'ordre ; aucune
  migration de schéma prévue (AD-19 « stockage absent ou vidé = page fonctionnelle » ne couvre pas « stockage d'une
  autre forme »).
- **Correction (AD-3).** Schéma versionné
  `aion2.links.v1 = { [eventId]: { organizer?, join?, participants: { [candidatureId]: secret } } }`, un seul module
  `web/src/api/links.ts` lit et écrit, et le DTO participant porte `candidatureId`.

### P15 : ordre de build, `rust-embed` exige `web/dist` avant le premier `cargo` (basse)

- **Unité A : epic 8, story « workflow CI ».** Suit l'ordre d'AD-10 : `cargo fmt`, `clippy`, `test`, `cargo tree`,
  `validate-reference`, `check-hardening.sh`, puis `tsc`, `vitest`, `vite build`.
- **Unité B : epic 1, story « servir la SPA ».** `#[derive(RustEmbed)] #[folder = "../../web/dist"]` ; « le build web
  est une dépendance du build serveur en release » : B ajoute un `build.rs` qui lance `npm ci && vite build` en profil
  release.
- **Collision.** En CI, `cargo clippy` échoue avant `vite build` (`rust-embed` exige que le dossier existe à la
  compilation, même en debug) ; dans le Dockerfile, l'étape `rust` n'a pas Node et le `build.rs` de B échoue, ou B
  installe Node dans l'étape Rust et le multi-étapes d'AD-1 n'a plus de sens.
- **Correction (AD-1, AD-10).** Le crate serveur n'invoque jamais npm ; la dépendance est un ordre : `vite build`
  précède toute commande `cargo` (CI et Dockerfile), `web/dist/` est copié depuis l'étape `node`, et un
  `web/dist/.gitkeep` versionné laisse compiler sans build.

## Synthèse

| Paire | Zone | Sévérité | AD à resserrer |
| --- | --- | --- | --- |
| P1 | forme de `assignments`, `apply_placement` delta ou complet, placement stocké ou dérivé | critique | AD-4 |
| P2 | retrait d'un joueur placé, transitions de `status`, origine du lien, candidature en `locked` | critique | AD-4 |
| P3 | `expired` dérivé ou matérialisé, réponse tardive, demande par candidature ou par événement | haute | AD-4, AD-14 |
| P4 | secret d'un autre droit sur une route, client qui envoie un secret du `localStorage` | haute | AD-3 |
| P5 | `/api/health` sans en-tête IP, classes de route, sous-commande `healthcheck` | haute | AD-7, AD-8, AD-10 |
| P6 | copie non atomique de `reference/`, mtime mémorisé avant validation, bind mount hors compose | haute | AD-6, AD-10 |
| P7 | instantané de référence, `enabled = false`, disposition figée au verrouillage | haute | AD-6, AD-4, AD-18 |
| P8 | jeton sans état ou à usage unique, qui compte pour le disjoncteur | moyenne | AD-7, AD-14 |
| P9 | cycle de vie des groupes au déverrouillage, créneau supprimé | moyenne | AD-4, AD-18 |
| P10 | forme textuelle des instants, `datetime('now')`, trois prédicats de temps | moyenne | AD-11 |
| P11 | `visibility` écrite par l'organisateur et l'administration | moyenne | AD-4, ADMIN |
| P12 | taxonomie des codes, clés `fields`, couverture des locales | moyenne | AD-5 |
| P13 | ETag manuel sous `no-store`, rechargement de la SPA après déploiement | moyenne | AD-5, AD-9 |
| P14 | schéma `localStorage` des liens | basse | AD-3 |
| P15 | `web/dist` avant `cargo`, pas de `build.rs` npm | basse | AD-1, AD-10 |

Quinze paires : deux critiques, cinq hautes, six moyennes, deux basses. Aucune ne demande un AD nouveau ni une
décision produit ; P2 (candidature en `locked`, origine du lien) et P9 (ce que garde un déverrouillage) méritent un
mot de Vincent parce qu'elles touchent au comportement visible de l'organisateur.
