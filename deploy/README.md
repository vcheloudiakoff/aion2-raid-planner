# Déploiement sur le NAS

Procédure pas à pas pour mettre l'outil en ligne et l'exploiter (AD-2, AD-9, AD-10, AD-14). Tout ce qui
suit se fait à la main, sur GitHub, le NAS, le VPS et le DNS ; rien de tout cela n'est automatisé par le
dépôt.

Conventions de ce document : `raid.example.org` est le nom public de l'outil, `<OWNER>` le compte GitHub
propriétaire du dépôt, `VPS_TUNNEL_IP` et `NAS_TUNNEL_IP` les adresses WireGuard du VPS et du NAS dans
le tunnel. Les vraies valeurs ne s'écrivent que sur les machines, jamais dans le dépôt ni dans un commit.

## Ce que fait un déploiement

Chaque push sur `main` (ou un lancement manuel du workflow `deploy` sur `main`) :

1. Le job `ci` tourne sur GitHub : tests, contrôles, image construite, conteneur de fumée (y compris une
   sauvegarde puis une restauration).
2. S'il est vert, le job `deploy` tourne sur le runner du NAS, sans `actions/checkout` ni secret GitHub :
   - si le commit n'est plus la tête de `main` (un push plus récent existe, ou ancien run relancé), le
     job s'arrête là, vert, avec une notice *Superseded* : c'est le commit le plus récent qui déploie ;
   - le clone `~/aion2-raid-planner` est remis exactement sur le commit testé par `ci` (fichiers non
     suivis supprimés, `deploy/.env` gardé) ;
   - `deploy/plan-deploy.sh` compare la révision de l'image en place (label
     `org.opencontainers.image.revision`) à ce commit :
     - une migration existante modifiée, supprimée ou renommée : job rouge, conteneur intact ;
     - une migration ajoutée : sauvegarde `pre-deploy` dans le conteneur en place avant toute
       construction ;
     - un fichier hors de `reference/` modifié : `docker compose up -d --build` ;
     - seulement `reference/` : pas de construction, mais `docker compose up -d` relance un conteneur
       arrêté ;
   - dans tous les cas, `docker compose cp reference/. server:/data/reference/` (rechargée à chaud en
     moins d'une minute) ;
   - dix essais de `GET /api/health` ; après construction, `X-App-Build` doit valoir le commit déployé.
3. En cas d'échec, le job est rouge et ses journaux montrent ceux du conteneur.

## 1. Prérequis sur le NAS

Vérifier, sur le NAS :

```sh
docker version --format '{{.Server.Version}}'   # Engine 25 ou plus
docker compose version --short                  # Compose 2.24 ou plus
caddy version                                   # Caddy 2 en service de l'hôte
git --version && curl --version | head -n 1 && bash --version | head -n 1
```

Il faut aussi : le tunnel WireGuard VPS ↔ NAS en place (les deux adresses de tunnel connues), et un
port local libre pour le conteneur (8080 par défaut, réglable par `PORT` dans `deploy/.env`).

## 2. Réglages GitHub

Dans le dépôt sur GitHub :

1. **Settings → Actions → General → Approval for running fork pull request workflows from
   contributors** : choisir *Require approval for all external contributors*. Le workflow n'a de toute
   façon ni déclencheur `pull_request` ni de fork (`check-hardening.sh` le refuse en CI).
2. **Settings → Actions → Runners** : le runner sera un runner *de dépôt* (pas d'organisation), affecté
   à ce seul dépôt. Ne pas le partager avec un autre dépôt.
3. Aucun secret à créer : le job `deploy` n'en utilise aucun (`permissions: {}`).

## 3. Utilisateur dédié et runner

Le runner tourne en service systemd sous un utilisateur dédié `aion2-deploy`, membre du groupe `docker`
(ce qui équivaut à root sur le NAS : c'est le compromis accepté par AD-10, comme pour les runners
existants). Jamais de runner en conteneur avec le socket Docker.

```sh
sudo useradd --create-home --shell /bin/bash --groups docker aion2-deploy
sudo -iu aion2-deploy
mkdir ~/actions-runner-aion2-raid-planner && cd ~/actions-runner-aion2-raid-planner
```

Puis, dans **Settings → Actions → Runners → New self-hosted runner** (Linux, architecture du NAS),
copier les commandes *Download* affichées (téléchargement et contrôle de somme) et les lancer dans ce
dossier. Pour *Configure*, utiliser le jeton affiché (valable une heure, à ne coller nulle part
ailleurs) avec ces options :

```sh
./config.sh --url https://github.com/<OWNER>/aion2-raid-planner --token <JETON> \
    --name nas-aion2 --labels nas --unattended
exit   # retour à l'utilisateur d'administration
```

Le runner reçoit les labels `self-hosted`, `Linux`, l'architecture et `nas` : le job `deploy` demande
`[self-hosted, nas]`. Installer et démarrer le service sous `aion2-deploy` :

```sh
cd ~aion2-deploy/actions-runner-aion2-raid-planner
sudo ./svc.sh install aion2-deploy
sudo ./svc.sh start
sudo ./svc.sh status
```

Le runner doit apparaître *Idle* dans **Settings → Actions → Runners**.

## 4. Clone et `.env`

Toujours sous `aion2-deploy` (`sudo -iu aion2-deploy`) :

```sh
git clone https://github.com/<OWNER>/aion2-raid-planner.git ~/aion2-raid-planner
cd ~/aion2-raid-planner
umask 077
cp deploy/.env.example deploy/.env
chmod 600 deploy/.env
# Clé HMAC du jeton de formulaire : 48 octets aléatoires, jamais affichés ni copiés ailleurs.
secret=$(head -c 48 /dev/urandom | base64 | tr -d '\n')
sed -i "s|^FORM_TOKEN_SECRET=.*|FORM_TOKEN_SECRET=$secret|" deploy/.env
unset secret
```

Éditer ensuite `deploy/.env` : `PUBLIC_BASE_URL=https://raid.example.org` (le vrai nom), et `PORT` si
8080 est déjà pris sur le NAS. Les autres variables gardent leur défaut. Vérifier que le compose est
complet sans afficher les valeurs :

```sh
docker compose -f deploy/docker-compose.yml config --quiet && echo OK
```

`deploy/.env` n'est pas versionné ; le `git reset --hard` du job `deploy` ne le touche pas.

## 5. Premier déploiement

Dans **Actions → deploy → Run workflow**, branche `main`. Attendu :

- `ci` vert, puis `deploy` sur le runner du NAS ;
- un avertissement *Premier déploiement* : aucune image en place, donc construction complète, sans
  garde-fou de migrations ni sauvegarde préalable ;
- la construction sur le NAS prend plusieurs minutes la première fois (le cache sert ensuite) ;
- l'étape *Santé* finit par `healthy`.

Sur le NAS :

```sh
cd ~aion2-deploy/aion2-raid-planner
docker compose -f deploy/docker-compose.yml ps          # STATUS : healthy
curl -fsS -D - -o /dev/null http://127.0.0.1:8080/api/health
```

Un push sur `main` fait la même chose. Un run dont le commit n'est plus la tête de `main` finit vert
avec une notice *Superseded* sans rien toucher : c'est normal. Tant que le runner n'existe pas, le job `deploy` reste en attente
puis échoue ; ce n'est pas grave.

## 6. Caddy du NAS

Ajouter au Caddyfile de l'hôte le bloc de `deploy/caddy/nas.caddy`, en remplaçant `raid.example.org`,
`VPS_TUNNEL_IP` et, si besoin, le port. Le bloc partage `:80` avec les autres sites `http://` du NAS.
En premier, `@nottunnel` coupe la connexion de tout client autre que le VPS dans le tunnel, LAN
compris. Aucune directive `log` dans ce bloc.

```sh
caddy validate --config /chemin/du/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
# Depuis le NAS lui-même (adresse qui n'est pas celle du tunnel) : la connexion doit être coupée.
curl -sS -o /dev/null -H 'Host: raid.example.org' http://127.0.0.1/ ; echo "curl: $?"   # 52 attendu
```

## 7. Caddy du VPS

Ajouter au Caddyfile du VPS le bloc de `deploy/caddy/vps.Caddyfile`, en remplaçant `raid.example.org`
et `NAS_TUNNEL_IP`. Il termine le TLS, compresse, pose HSTS, retire `Server`, écrase `X-Client-IP` avec
l'adresse du client et relaie toutes les méthodes vers le NAS. Aucune directive `log` dans ce bloc. Tous
les autres en-têtes viennent de l'application : ne pas les ajouter ici.

```sh
caddy validate --config /chemin/du/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
# Depuis le VPS, à travers le tunnel : 200 attendu.
curl -sS -o /dev/null -w '%{http_code}\n' -H 'Host: raid.example.org' http://NAS_TUNNEL_IP/api/health
```

## 8. DNS

Dans la zone DNS du nom public (une zone **sans aucun enregistrement vers le domicile**, et jamais la
zone du NAS) : un enregistrement `A` (et `AAAA` si le VPS a de l'IPv6) de `raid.example.org` vers
l'adresse publique du VPS. Rien d'autre. Au premier accès HTTPS, Caddy obtient le certificat.

## 9. Vérifications

Depuis un poste quelconque :

```sh
curl -sS -D - -o /dev/null https://raid.example.org/api/health
curl -sS -D - -o /dev/null https://raid.example.org/
```

Attendu (AD-9) :

- statut `200` sur `/api/health` ;
- `strict-transport-security: max-age=31536000` ;
- `content-security-policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'
  data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'` ;
- `referrer-policy: no-referrer` et `x-content-type-options: nosniff` ;
- `x-app-build` égal au commit déployé et `cache-control: no-store` sur `/api/*` ;
- aucun en-tête `server`.

Depuis le LAN, une requête directe au NAS avec le nom public doit être coupée (voir §6). Ouvrir enfin
l'URL publique sur un téléphone.

## 10. Sauvegardes

Le serveur fait lui-même ses sauvegardes, dans le volume, en `/data/backups/` (pas de cron du NAS, pas
de conteneur de plus) :

- **quotidienne** : chaque heure, si la dernière `daily-*` a plus de 24 h, `VACUUM INTO` vers
  `daily-YYYYMMDDTHHMMSSZ.db` ; les deux plus récentes sont gardées ;
- **avant déploiement** : quand un push ajoute une migration, le job lance
  `backup pre-deploy` avant de construire (`pre-deploy-YYYYMMDDTHHMMSSZ.db`), supprimée après deux jours.

Chaque fichier est écrit sous un nom temporaire puis renommé : un `.db` listé est toujours complet. Les
sauvegardes vivent sur le même volume que la base : elles protègent d'une mauvaise migration ou d'une
erreur, pas d'une perte du disque (pas de sauvegarde hors NAS pour l'instant).

```sh
cd ~aion2-deploy/aion2-raid-planner
c="docker compose -f deploy/docker-compose.yml"
$c run --rm --no-deps server restore               # liste les sauvegardes (code de sortie 2)
$c exec -T server /server backup pre-deploy        # sauvegarde manuelle, nom affiché
```

L'image n'a ni shell ni `sqlite3` : tout passe par les sous-commandes `backup` et `restore` du binaire,
dans la même image, sous le même utilisateur.

## 11. Restauration

À jouer une fois après le premier déploiement, pour la connaître.

```sh
cd ~aion2-deploy/aion2-raid-planner
c="docker compose -f deploy/docker-compose.yml"
$c run --rm --no-deps server restore               # 1. choisir un nom dans la liste
$c stop server                                     # 2. service arrêté, obligatoire
$c run --rm --no-deps server restore pre-deploy-YYYYMMDDTHHMMSSZ.db   # 3. restaurer
$c up -d --no-build --wait                         # 4. redémarrer
curl -fsS http://127.0.0.1:8080/api/health         # 5. vérifier
```

L'étape 3 contrôle l'intégrité de la sauvegarde, la copie à côté de `app.db`, supprime `app.db-wal` et
`app.db-shm`, puis remplace `app.db` d'un seul renommage. Nom inconnu ou fichier corrompu : code non
nul et `app.db` intact. Ne jamais lancer l'étape 3 service démarré.

Si la sauvegarde visée a presque deux jours, ne pas lancer de `backup` juste avant : la rotation la
supprimerait. Une sauvegarde antérieure à une migration se restaure telle quelle : le serveur rejoue les
migrations manquantes au démarrage.

## 12. Base inconnue

Le job `deploy` échoue avec `unknown base` quand la révision de l'image du conteneur en place n'est pas
un ancêtre du commit à déployer : historique de `main` réécrit (force-push), ou image construite à la
main sans `APP_BUILD`. Le conteneur en place n'a pas été touché.

Ne pas confondre avec un run ancien (son `ci` a fini après celui d'un push plus récent, ou il a été
relancé) : celui-là finit vert avec une notice *Superseded* et ne demande rien.

1. Vérifier à la main qu'aucune migration existante n'a changé depuis ce qui tourne :
   `git log --oneline -- crates/server/migrations/` dans le clone, comparé à la révision en place
   (`docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}'
   aion2-raid-planner:local`).
2. Si une migration déjà appliquée a changé : ne pas déployer, corriger `main` (migration de
   compensation).
3. Sinon, reconstruire à la main sur le commit voulu, avec sa révision :

   ```sh
   cd ~aion2-deploy/aion2-raid-planner
   c="docker compose -f deploy/docker-compose.yml"
   git fetch origin main && git reset --hard origin/main
   $c exec -T server /server backup pre-deploy
   APP_BUILD=$(git rev-parse HEAD) $c up -d --build
   $c cp reference/. server:/data/reference/
   ```

4. Relancer le workflow sur `main` : la base est maintenant connue.

Toute construction à la main passe `APP_BUILD=$(git rev-parse HEAD)` ; sans lui, l'image porte `dev` et
le déploiement suivant s'arrête sur `unknown base`.

## 13. Retour arrière

Revert du commit fautif et push sur `main` : le déploiement suivant remet l'état précédent. Jamais de
force-push (il mène au §12).

Exception : un commit qui ajoute une migration ne se revert pas (migrations forward-only, et le
garde-fou refuserait la suppression). Le retour arrière est une nouvelle migration de compensation, ou
en dernier recours une restauration de la sauvegarde `pre-deploy` faite juste avant (§11) suivie d'un
correctif sur `main`. Cette sauvegarde est supprimée deux jours après sa création : ce recours n'existe
que pendant ce délai (au-delà, il ne reste que les deux quotidiennes).

## 14. Vérification de fin d'epic 1

Le signal de succès de l'epic 1 (`_bmad-output/specs/spec-epic-1-socle/SPEC.md`), pas à pas, après le
dernier push de l'epic.

1. **Accueil sur téléphone.** Ouvrir `https://raid.example.org/` sur un téléphone : l'accueil vide
   s'affiche. Ouvrir *Langue · Thème*, passer en English puis revenir en Français : les deux langues
   s'affichent.
2. **Santé.** Depuis un poste quelconque :

   ```sh
   curl -sS -o /dev/null -w '%{http_code}\n' https://raid.example.org/api/health   # 200
   ```

3. **Référence sans rebuild.** Dans `reference/classes.toml`, changer le libellé `fr` (ou `en`) d'une
   classe, commit, push sur `main`. Le job `deploy` ne reconstruit pas l'image (seul `reference/` a
   changé). Moins d'une minute après la fin du job :

   ```sh
   curl -sS https://raid.example.org/api/reference | grep -o 'NOUVEAU LIBELLÉ'
   ```

   Remettre ensuite le libellé d'origine par un second push.
4. **IP visiteur (AD-7).** Via le VPS, la route à clé répond, parce que le Caddy du VPS pose
   `X-Client-IP` :

   ```sh
   curl -sS -o /dev/null -w '%{http_code}\n' https://raid.example.org/api/reference   # 200
   ```

   Sur le NAS, sur le port local, sans l'en-tête : refus, jamais de repli sur l'adresse du pair.

   ```sh
   curl -sS -w '\n%{http_code}\n' http://127.0.0.1:8080/api/reference
   # 400 et "code":"limit.client_ip_missing"
   curl -sS -o /dev/null -w '%{http_code}\n' -H 'X-Client-IP: 127.0.0.1' \
       http://127.0.0.1:8080/api/reference                                          # 200
   ```

   Un 400 via le VPS signale que `header_up X-Client-IP {remote_host}` manque dans son Caddyfile
   (§7).
5. **Relecture du compose déployé.** Sur le NAS :

   ```sh
   cd ~aion2-deploy/aion2-raid-planner
   docker compose -f deploy/docker-compose.yml config
   ```

   Vérifier, contre l'intent §4 : `user: "10001:10001"`, `read_only: true`, `cap_drop: [ALL]`,
   `no-new-privileges:true`, ni `privileged`, ni `pid: host`, ni `network_mode: host`, ni socket
   Docker, ni montage d'un répertoire du NAS (seul le volume nommé `/data`), port publié sur
   `127.0.0.1` seulement, secrets venus de `deploy/.env` (droits `600`).
6. **Relecture des en-têtes.** Refaire les deux `curl -D -` du §9 et vérifier la liste attendue :
   HSTS, CSP exacte, `referrer-policy`, `x-content-type-options`, `x-app-build` égal au commit,
   `cache-control: no-store` sur `/api/*`, aucun en-tête `server`.
