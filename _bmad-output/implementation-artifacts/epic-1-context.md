# Epic 1 Context: Socle

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Poser le socle technique sur lequel les epics 2 à 5 ne feront qu'ajouter des routes et des écrans : un
binaire Rust unique qui sert l'API et la SPA React embarquée, une base SQLite, une référence du jeu en TOML
rechargée à chaud, un conteneur durci déployé par push sur `main`, et les deux mécanismes indispensables
à un outil sans compte, à savoir les droits par liens secrets et l'anti-abus. L'outil est public et tourne
sur le NAS personnel de Vincent : il ne doit jamais devenir un tremplin vers ce NAS ni révéler le
domicile. L'utilisateur de cet epic est Vincent, en exploitant ; aucun écran métier n'est livré.

## Stories

- Story 1.1: Squelette du binaire, base SQLite et santé
- Story 1.2: Référence du jeu en TOML, rechargée à chaud
- Story 1.3: Conteneur durci et CI complète
- Story 1.4: Déploiement par runner, edge et sauvegardes
- Story 1.5: Coquille SPA bilingue et contrat d'erreurs
- Story 1.6: Droits par liens secrets
- Story 1.7: Anti-abus sans compte

## Requirements & Constraints

- Santé : `GET /api/health` renvoie 200 si base ouverte, migrations à jour et référence chargée, sinon 503 ; la sous-commande `healthcheck` appelle la même `app::health()`.
- Référence : un commit qui ne touche que `reference/` est visible dans `GET /api/reference` en moins de deux minutes, sans rebuild ; un TOML invalide laisse l'ancien jeu en service et journalise l'erreur.
- Isolation non négociable : non-root (`10001:10001`), `read_only`, `cap_drop: ALL`, `no-new-privileges`, limites mémoire/CPU, port lié à `127.0.0.1` seulement ; jamais `privileged`, `pid`, socket Docker, `cap_add`, `network_mode: host` ni bind mount.
- Aucune connexion sortante : aucune crate client HTTP, SMTP ou DNS (`reqwest`, `ureq`, `lettre`, `hickory`, `hyper-util` feature `client`), contrôlé en CI. Seule exception : la requête `healthcheck` écrite à la main sur `TcpStream` vers `127.0.0.1`.
- Dépôt public : aucun secret, IP, nom d'hôte ni donnée de joueur, y compris dans `deploy/`, les workflows et les exemples Caddy (placeholders). `.env` sur le NAS seulement ; `deploy/.env.example` sans valeur secrète.
- Aucun secret de lien en clair en base, en URL d'API, en cookie ou en journal ; aucune IP en base ni en journal en production (test sur la sortie `tracing`). Aucun corps de requête journalisé ; `RUST_LOG=debug` interdit en production.
- Aucune valeur de référence du jeu dans le code Rust ou React.
- Bundle : dépendances d'exécution limitées à React, react-dom, react-router, Tailwind ; JS initial ≤ 150 ko gzip, CSS ≤ 20 ko gzip, contrôlés en CI.
- Aucun workflow `pull_request` ni de fork n'atteint le runner auto-hébergé.
- Hors périmètre : écrans métier, administration et `creation_enabled`, purge, preuve de travail (ni feature Cargo ni solveur), staging, E2E navigateur, PWA, mesure de performance en CI. Les gestes sur le NAS, le VPS et le DNS sont faits par Vincent, guidés par une procédure écrite.
- Signal de succès : l'URL publique affiche l'accueil vide en fr et en en sur téléphone, `/api/health` est vert, un changement de libellé dans `reference/` apparaît après push sans rebuild, et une relecture du compose et des en-têtes ne trouve aucun point interdit.

## Technical Decisions

- **Couches** : workspace Cargo `crates/domain` (noyau pur, sans I/O, sans sqlx/axum/tokio) et `crates/server` (`http → app → infra`, sans trait de dépôt). Seuls `Clock` et le générateur d'ids et de secrets sont des traits injectés. `web/` ne connaît que le contrat JSON via `web/src/api/types.ts`.
- **Binaire unique** : sert `/api/*` et `web/dist/` embarqué par `rust-embed` ; toute route GET sans extension renvoie `index.html`, asset absent = 404 ; pas de CORS. `vite build` précède toute commande `cargo` ; `web/dist/.gitkeep` versionné pour que `cargo check` passe sans build web. Sous-commandes : `healthcheck`, `validate-reference`, `list-error-codes`.
- **Config** : variables d'environnement lues une fois dans `Config`, fail-fast, pas de chargeur `.env`. Obligatoires : `PUBLIC_BASE_URL`, `FORM_TOKEN_SECRET` (≥ 32 octets). Défauts : `PORT` 8080 (le conteneur écoute toujours 8080), `CLIENT_IP_HEADER` `X-Client-IP`, `TRUST_CLIENT_IP_HEADER` `true`, `MIN_SUBMIT_DELAY_MS` 3000, `MAX_EVENTS_PER_HOUR` 30, `MAX_CANDIDATURES_PER_HOUR` 300, `RATE_CREATE`/`RATE_CANDIDATURE`/`RATE_WRITE`/`RATE_READ` 5/h, 20/h, 120/min, 600/min, `RUST_LOG` `info`. Chemins fixes : `/data/app.db`, `/data/reference/`, `/data/backups/`.
- **SQLite** : SQLx sur `/data/app.db`, pragmas `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000` ; pool d'écriture à 1 connexion, lecture à 4. Migrations `sqlx::migrate!` embarquées, appliquées au démarrage, forward-only et immuables une fois sur `main`. Tests HTTP sur `sqlite::memory:`.
- **Temps** : `domain::Timestamp`, forme unique `YYYY-MM-DDTHH:MM:SSZ` (implémente `sqlx::Type` et `serde`) ; jamais `String` pour un instant, jamais `datetime('now')` ni `CURRENT_TIMESTAMP` (grep en CI).
- **Référence** : cinq fichiers `regions`, `servers`, `instances`, `classes`, `roles` (`.toml`), slugs anglais jamais renommés, retrait par `enabled = false`, `label.en` obligatoire et `fr` avec repli. Libellés de faction dans un tableau `[factions]` en tête de `servers.toml`. Rechargement à chaque tick (≤ 60 s) si un mtime change : validation complète, remplacement atomique d'un `Arc<ReferenceSet>`, mtime mémorisé seulement après succès. Le serveur n'écrit jamais dans `/data/reference/` ; le jeu embarqué ne sert que si le dossier est vide. Même validateur au démarrage, au rechargement et en CI. `GET /api/reference` avec ETag, entrées désactivées incluses. Schéma, contrôles et contenu initial (marqué `[ASSUMPTION]`, à saisir tel quel) dans `reference-schema.md` de la spec.
- **Contrat HTTP** : DTO dans `server::http::dto`, `camelCase`. Erreurs `application/problem+json` avec `code = <entité>.<cas>` issu de l'enum `http::errors::Code` (entités : `event, slot, candidature, placement, confirmation, reference, auth, form, limit, admin`) ; 400 validation, 409 conflit, 401/403/404 droit, 429/503 avec `Retry-After`, 5xx sans détail. Le serveur n'émet aucune phrase destinée à l'utilisateur. Chaque réponse `/api/*` porte `X-App-Build`.
- **En-têtes posés par le binaire** (pas par Caddy) : CSP `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `Permissions-Policy` minimale, `Cache-Control` (assets hachés immutables, API `no-store`), `X-Robots-Tag: noindex` sur les chemins secrets. L'edge VPS gère TLS, HSTS, compression, retrait de `Server` et pose `X-Client-IP` en écrasant la valeur cliente ; le Caddy du NAS refuse toute IP autre que celle du tunnel (`handle @nottunnel { abort }`) puis `reverse_proxy` ; aucune directive `log`.
- **Liens secrets** : secrets de 256 bits (CSPRNG, base64url sans padding, 43 caractères), ids publics de 128 bits (22 caractères). Seul `SHA-256(secret)` est stocké, recherche par hash. Secret reçu uniquement par l'en-tête `X-Link-Secret`, ignoré en query string et cookie. Extracteur unique `LinkSecret` par droit et portée : absent = 401 `auth.missing`, inconnu = 404, autre droit ou autre portée = 403 `auth.wrong_right`. GET et HEAD sans effet. Prouvé sur des routes `cfg(test)`.
- **Anti-abus** : classes `create`, `candidature`, `write`, `read` dans `http::limits`, et `nokey` (health, assets, `index.html`) exemptée. Limiteur `governor` (sans `jitter`) en mémoire par IP, IPv6 groupé par /64, `retain_recent()` à chaque tick. IP lue dans l'en-tête mono-valeur `CLIENT_IP_HEADER`, jamais `X-Forwarded-For` ; en mode confiance, absent ou multi-valué = 400 `limit.client_ip_missing`, jamais de repli sur le pair. Jeton de formulaire sans état : HMAC sur `ts ‖ nonce ‖ classe`, émis par `GET /api/form-token`, exigé sur les POST `create` et `candidature`, âge entre `MIN_SUBMIT_DELAY_MS` et une heure, aucune table ; honeypot. Disjoncteur sur écritures commises → 503. Bornes de champs dans `domain::validate`.
- **Conteneur** : Dockerfile `node → rust (musl) → scratch`, `/data` vide copié `--chown=10001:10001`, `tmpfs: /tmp`, `pids_limit: 128`, `mem_limit: 256m`, `cpus: 1.0`, `restart: unless-stopped`, journaux `json-file` 10m × 3, réseau bridge dédié, un seul volume nommé en `/data`, variables obligatoires en `${VAR:?}`. `deploy/check-hardening.sh` testé par fixtures.
- **CI/CD** : `.github/workflows/deploy.yml` déclenché par `push` sur `main` et `workflow_dispatch`. Job `ci` : fmt, clippy `-D warnings`, test, contrôle `cargo tree -e features`, `cargo deny check advisories`, `validate-reference`, `list-error-codes` contre les locales, `check-hardening.sh`, `tsc --noEmit`, `vitest`, `npm audit --audit-level=high`, `vite build` + budget. Job `deploy` (`needs: ci`, `[self-hosted, nas]`, `concurrency: deploy-nas`) : `git reset --hard origin/main`, garde-fou sans `M` ni `D` dans `crates/server/migrations/`, `VACUUM INTO` si migration ajoutée, `docker compose up -d --build` sauf push limité à `reference/`, puis toujours `docker compose cp reference/. server:/data/reference/` et health-check (dix essais). Sauvegarde quotidienne `VACUUM INTO` (deux conservées) ; restauration = stop, copie sur `app.db`, suppression `-wal`/`-shm`, up. Dependabot hebdomadaire Cargo et npm.
- **Front** : `client.ts` unique avec conditionnel manuel (`etag` + corps mémorisés par URL, `If-None-Match`, 304 → corps mémorisé) et détection de `X-App-Build`. `t()` maison, `web/src/locales/fr.json` et `en.json` plats, mêmes clés, dont `errors.<code>`. Langue = `navigator.language` puis choix en `localStorage`. `links.ts` seul module de `localStorage` des liens, schéma `links.v1 = { [eventId]: { organizer?, join?, participants: { [candidatureId]: secret } } }`. Tout accès `localStorage` enveloppé. Pas de bibliothèque d'état ni de requêtes. Tests `vitest` sur locales, `client.ts`, `links.ts`.
- **Versions** : celles de la table *Stack* de la spine d'architecture (Rust 1.99.1 édition 2024, axum 0.8.9, sqlx 0.8.6, React 19.3.1, Vite 8.3.6, Tailwind 4.3.4, TypeScript 7.0.2, etc.).

## UX & Interaction Patterns

- Mobile d'abord à 360 px (gouttière 16 px), aucun défilement horizontal ; au-delà de 640 px, colonne centrée. Cibles tactiles ≥ 44 px.
- Visuel : tokens de `DESIGN.md` (surfaces, encres, un seul accent or), polices système uniquement, sombre par défaut, thème suivant `prefers-color-scheme` tant que rien n'est choisi. Aucun logo, image ni police du jeu.
- En-tête global : nom de l'outil à gauche, bouton à droite qui ouvre une feuille du bas « Langue · Thème » (fr/en, sombre/clair, choix mémorisé). Feuille : `role="dialog"`, `aria-modal`, focus piégé et rendu au déclencheur, fermeture par Échap, glissement ou bouton Fermer. Pas de menu hamburger.
- Accueil vide : « Aucun raid ouvert pour l'instant. » ; section « Sur cet appareil » masquée sans lien local. Pas de bouton « Créer un raid » actif avant l'epic 2.
- Premier rendu immédiat depuis la référence en cache `localStorage` (avec ETag), jamais de plein écran de chargement.
- Bandeau « Hors ligne · dernières données de HH:MM » si le serveur est injoignable ; bandeau « Nouvelle version · Recharger » sur changement de `X-App-Build`, sans rechargement forcé.
- Stockage local indisponible : la page marche quand même.
- Attribut `lang` du document suivant la langue choisie ; focus visible dans les deux thèmes ; `prefers-reduced-motion` respecté.
- Ton : bref, tutoiement en français, registre familier en anglais, sans excuse, sans point d'exclamation, sans jargon technique. Libellés de référence pris dans la référence (`fr` sinon `en`), jamais dans les locales.

## Cross-Story Dependencies

- 1.1 fonde tout : workspace, `Config`, pools SQLite, `Timestamp`, `Clock`, générateur, `app::health()`, en-têtes. 1.2 étend `app::health()` avec « référence chargée ».
- 1.3 (conteneur, CI) précède 1.4 (job `deploy`, edge, sauvegardes, premier déploiement). 1.4 commence par faire vérifier à Vincent les versions Docker Engine et Compose du NAS.
- Le job `ci` de 1.3 dépend de `validate-reference` (1.2) et de `list-error-codes` contre les locales (1.5) ; ces contrôles s'ajoutent quand leurs sous-commandes existent.
- 1.6 et 1.7 utilisent le registre d'erreurs (1.5) pour `auth.*` et `limit.*`, et le générateur d'ids et de secrets (1.1). 1.7 clôt l'epic par un déploiement vérifié contre le signal de succès.
- Vers l'aval : `LinkSecret`, les classes de limiteur, le jeton de formulaire et les seuils du disjoncteur reçoivent leurs premières routes réelles en epic 2 ; l'interrupteur `creation_enabled`, `ADMIN_SECRET` et la purge arrivent en epic 5.
- Questions ouvertes à ne pas trancher seul : format exact du tag `[SRV]` affiché par le client du jeu ; versions Docker du NAS ; conservation légale des données d'identification (AD-7 et AD-15 s'appliquent tels quels en attendant).
