# Revue — versions, réalité des outils et défauts réels (lens « vérifié contre la réalité »)

- **Artefact revu :** `ARCHITECTURE-SPINE.md` (statut `draft`, 2026-11-05), table « Stack » et crates/outils cités dans les AD.
- **Date de la vérification :** 2026-11-05, faite indépendamment de l'auteur, par `curl` sur l'API crates.io (avec User-Agent), le registre npm (`dist-tags` et `peerDependencies`), l'API GitHub (releases, tags, issues, fichiers bruts), Docker Hub (tags), `nodejs.org/dist/index.json`, le dépôt apt Cloudsmith de Caddy, la Compose Specification (`compose-spec`, `compose-go`) et la référence `docs.docker.com`. Rien n'a été compilé, aucun projet n'a été créé.

## Verdict

Toutes les versions de la table « Stack » sont exactes au 2026-11-05 et les deux affirmations techniques (binaire musl dans `scratch`, syntaxe Compose) tiennent ; en revanche trois décisions sont affirmées sans leur défaut réel : TypeScript 7 est incompatible avec `typescript-eslint` (et le repli indiqué, 5.9, est le mauvais), `altcha-lib-rs` est une implémentation communautaire du protocole ALTCHA v1 alors que l'écosystème et la crate officielle sont passés au protocole v2, et sqlx 0.9.0 est la seule release de sa série depuis mai avec des régressions SQLite connues et des correctifs fusionnés mais non publiés — à quoi s'ajoute un piège de déploiement non couvert par AD-8 (propriétaire du volume `/data` sur une image `scratch` non-root).

## Table de vérification

| Techno | Version dans la spine | Version constatée (2026-11-05) | Source | Verdict |
| --- | --- | --- | --- | --- |
| Rust (édition 2024, builder `rust:1.99-alpine`, cible musl) | 1.99.1 | 1.99.1 (release 2026-10-16) ; tag Docker Hub `1.99-alpine` et `1.99.1-alpine` publiés 2026-11-05 ; Dockerfile `stable/alpine3.23` : `RUST_VERSION=1.99.1`, `apk add musl-dev gcc`, hôte `x86_64-unknown-linux-musl` | GitHub `rust-lang/rust`, Docker Hub, `rust-lang/docker-rust` | OK |
| axum | 0.8.9 | 0.8.9 (2026-04-14) ; dépend de `http ^1`, `tower ^0.5.2`, `tokio ^1.44` | crates.io | OK |
| tokio | 1.54.1 | 1.54.1 (2026-10-21) | crates.io | OK |
| tower-http | 0.7.1 | 0.7.1 (2026-08-31) ; dépend de `http ^1`, `tower ^0.5` : compatible axum 0.8 | crates.io | OK |
| sqlx (`runtime-tokio`, `sqlite-bundled`, `chrono`, `migrate`) | 0.9.0 | 0.9.0 (changelog 2026-05-06, crates.io 2026-05-21), non yankée, MSRV 1.94 ; les quatre features existent (`sqlite-bundled = ["_sqlite", "sqlx-sqlite/bundled", ...]`, `chrono`, `migrate`, `runtime-tokio`) ; aucune 0.9.x depuis ; dépôt déplacé vers `transact-rs/sqlx` | crates.io (`/crates/sqlx/0.9.0`), GitHub | OK sur la version ; voir F3 sur l'état réel |
| rust-embed | 8.13.0 | 8.13.0 (2026-10-07) ; champ `repository` = `https://pyrossh.dev/repos/rust-embed` (forge personnelle), `github.com/pyrossh/rust-embed` répond 404 | crates.io, GitHub, pyrossh.dev | OK sur la version ; voir F6 |
| governor | 0.10.4 | 0.10.4 (2025-12-16) ; dépôt actif (push 2026-08-24, 36 issues ouvertes) | crates.io, GitHub | OK ; voir F5 |
| altcha-lib-rs (feature `pow`) | 0.3.3 | 0.3.3 (2026-04-07), « Community implementation », 24 étoiles, dernier push 2026-07-27. Crate **officielle** `altcha` 1.1.0 (2026-10-19, `altcha-org/altcha-lib-rs`, « official Rust implementation of the ALTCHA Proof-of-Work (v2) protocol ») | crates.io (recherche `altcha`), GitHub | Périmé sur le choix ; voir F4 |
| serde | 1.0.229 | 1.0.229 (2026-07-18) | crates.io | OK |
| toml | 1.2.0 | 1.2.0+spec-1.1.0 (2026-11-04) | crates.io | OK |
| chrono | 0.4.45 | 0.4.45 (2026-06-04) ; `sqlx-sqlite` 0.9.0 exige `chrono ^0.4.34` | crates.io | OK |
| rand | 0.10.3 | 0.10.3 (2026-09-20), `getrandom ^0.4` | crates.io | OK |
| sha2 / hmac | 0.11.1 / 0.13.0 | 0.11.1 (2026-10-12) / 0.13.0 (2026-03-29) ; tous deux sur `digest 0.11` | crates.io | OK |
| data-encoding | 2.11.1 | 2.11.1 (2026-08-03) | crates.io | OK |
| thiserror | 2.0.21 | 2.0.21 (2026-09-23) | crates.io | OK |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | 0.1.44 (2025-12-18) / 0.3.23 (2026-03-13) | crates.io | OK |
| dotenvy (conventions, dev seulement) | non versionné | 0.15.7 (2023-03-22) ; dépôt vivant (push 2026-08-18) mais aucune release depuis 3 ans et demi, 49 issues ouvertes | crates.io, GitHub | OK ; voir F7 |
| Node (image `node:24-alpine`) | 24 LTS (24.22.2) | v24.22.2 « Krypton » LTS (2026-10-27) ; tag `node:24-alpine` publié 2026-10-28 | nodejs.org, Docker Hub | OK |
| React / react-dom | 19.3.1 | 19.3.1 `latest` (2026-10-14) | npm | OK |
| Vite / @vitejs/plugin-react | 8.3.6 / 6.2.1 | 8.3.6 `latest` (publié le 2026-11-05 même), engines `node ^20.19 \|\| >=22.12`, bundler `rolldown ~1.2.16` (aucune peer dep `typescript`) / 6.2.1 `latest` (2026-11-02), peer `vite ^8.0.0` | npm | OK |
| Tailwind CSS / @tailwindcss/vite | 4.3.4 | 4.3.4 / 4.3.4 `latest` (2026-11-02) ; `@tailwindcss/vite` peer `vite ^5.2 \|\| ^6 \|\| ^7 \|\| ^8` | npm | OK |
| TypeScript (`[ASSUMPTION]` ligne 7, repli 5.9.x) | 7.0.2 | 7.0.2 est bien `latest` (2026-07-08 ; `next` 7.1.0-dev, `beta` 7.1.0-beta) ; c'est le port natif Go (`bin/tsc` + binaires `@typescript/typescript-<os>-<arch>`), sans API programmatique stable avant 7.1. Dernière 5.x : 5.9.3 (2025-09-30) ; pont officiel : 6.0.3 (2026-04-16) et paquet `@typescript/typescript6` 6.0.2 (`tsc6`) | npm, blog « Announcing TypeScript 7.0 », GitHub `microsoft/typescript-go` | Version OK, **compatibilité non vérifiée** ; voir F1 |
| typescript-eslint (implicite derrière `eslint` 10) | — | 8.72.1 `latest` ; peer `eslint ^8.57 \|\| ^9 \|\| ^10` (OK) mais `typescript >=4.8.4 <6.1.0` (**exclut 7.0.2**, et même 6.1) | npm, `typescript-estree/src/parseSettings/warnAboutTSVersion.ts` | Conflit ; voir F1 |
| react-router (mode bibliothèque) | 8.5.1 | 8.5.1 `latest` (2026-10-27) ; v8.0.0 du 2026-06-17 : ESM-only, Node ≥ 22.22, React ≥ 19.2.7, `@types/react` ≥ 19.2 (19.3.0 dispo), **`react-router-dom` supprimé** (figé en 7.18.5) ; le mode déclaratif (`BrowserRouter`, `Routes`) reste documenté et exporté dans la référence v8 | npm, `CHANGELOG.md` v8.0.0, reactrouter.com | OK (le mode « bibliothèque » s'appelle « Declarative mode » dans la doc v7/v8) |
| vitest / eslint | 5.1.0 / 10.14.0 | 5.1.0 `latest` (2026-11-02), peer `vite ^6.4 \|\| ^7 \|\| ^8`, engines `node ^22.12 \|\| ^24 \|\| >=26` / 10.14.0 `latest` (2026-10-30), engines `node ^20.19 \|\| ^22.13 \|\| >=24` | npm | OK |
| Caddy (paquet apt stable, sans module, VPS et NAS) | 2.11.9 | v2.11.9 publiée **le 2026-11-05 même** (correctif d'une régression de 2.11.8 + failles FastCGI) ; déjà présente dans le dépôt apt Cloudsmith `caddy/stable` (2.11.7, 2.11.8, 2.11.9) | GitHub `caddyserver/caddy`, Cloudsmith `Packages` | OK ; voir F9 |
| Docker Engine / Compose (spec sans clé `version`) | 29.x / v5 | moby `docker-v29.9.3` (2026-11-03) / Compose v5.7.0 (2026-11-02 ; v5.0.0 du 2025-12-02 a sauté 3 et 4) ; clé `version` « obsolete », simple avertissement | GitHub `moby/moby`, `docker/compose`, `compose-spec/04-version-and-name.md` | OK |
| Image d'exécution | `scratch` (0 paquet) | voir affirmation (a) ci-dessous | — | OK avec deux réserves (F2, F10) |

### Affirmation (a) — binaire Rust musl statique avec sqlx `sqlite-bundled` dans `FROM scratch`

**Confirmée.** Le Dockerfile officiel `rust:1.99-alpine` (`rust-lang/docker-rust`, `stable/alpine3.23`) installe `musl-dev` et `gcc`, prend `x86_64-unknown-linux-musl` (ou `aarch64-…-musl`) comme hôte par défaut et **ne pose aucun `RUSTFLAGS=-C target-feature=-crt-static`** (l'ancien contournement a disparu) ; la cible musl de Rust lie en statique par défaut (`+crt-static`), donc `cargo build --release` produit un exécutable statique (static-pie) sans dépendance de bibliothèque. `sqlx-sqlite/bundled` active `libsqlite3-sys/bundled`, qui compile le `sqlite3.c` embarqué avec la crate `cc` : `gcc` + `musl-dev` de l'image suffisent. Rien dans la spine n'exige ce que `scratch` n'a pas : pas de certificats CA (AD-17), pas de base de fuseaux (UTC seul, AD-11), pas d'`/etc/passwd` (uid numérique). Réserves : le propriétaire du point de montage `/data` (F2) et le répertoire temporaire de SQLite (F10).

### Affirmation (b) — `${VAR:?message}`, `mem_limit`, `cpus`, `pids_limit` sans clé `version`

**Confirmée.** `compose-spec/12-interpolation.md` : « `${VARIABLE:?err}` exits with an error message containing `err` if VARIABLE is unset or empty ». Le motif de `compose-go/template/template.go` est `[_a-z][_a-z0-9]*(?::?[-+?](.*))?` : le message peut être vide, donc `${VAR:?}` tel qu'écrit dans AD-8 est accepté et produit « required variable VAR is missing a value ». `mem_limit`, `cpus` (fractionnaire) et `pids_limit` sont des attributs de service de la Compose Specification (`compose-go/types/types.go` : `MemLimit yaml:"mem_limit"`, `CPUS yaml:"cpus"`, `PidsLimit yaml:"pids_limit"` ; référence docs.docker.com « services ») et sont appliqués par Compose v2/v5 hors Swarm. `deploy.resources.limits.{cpus,memory,pids}` est l'autre forme ; la référence précise seulement que si les deux sont posées elles doivent être cohérentes. Il n'est donc **pas** nécessaire de passer par `deploy.resources.limits`, et `check-hardening.sh` peut bien exiger `mem_limit` et `cpus`. La clé `version` est « obsolete » (avertissement, jamais une erreur). La « question ouverte NAS » de la section Deferred reste pertinente uniquement pour la version réellement installée sur le NAS, pas pour la syntaxe.

## Findings

### Critique

Aucun.

### Haute

**F1 — TypeScript 7.0.2 est incompatible avec `typescript-eslint` ; le repli « 5.9.x » est le mauvais pont.**
`typescript-eslint` 8.72.1 (`latest`) déclare `peerDependencies.typescript: ">=4.8.4 <6.1.0"` et son `typescript-estree` encode la même borne en dur ; avec `typescript@7.0.2` dans le même `package.json`, `npm install` échoue en `ERESOLVE` (ou le lint tourne avec avertissement et un AST non supporté). Le blog d'annonce de TS 7 le dit explicitement : « TypeScript 7 does not yet expose a stable programmatic API … we have made it a priority to ensure TypeScript can be run side-by-side with TypeScript 6.0 for utilities that still need some programmatic access to the compiler (such as typescript-eslint) », via le paquet `@typescript/typescript6` (`tsc6`) et un alias npm. Par ailleurs TS 7 « adopts 6.0's new defaults, and provides hard errors in the face of any flags and constructs deprecated in TypeScript 6.0 » (`baseUrl` supprimé, `rootDir` à poser, `stableTypeOrdering`) : le pont documenté est **6.0.3**, pas 5.9.3. `vite build` (rolldown/oxc) et `vitest` ne dépendent pas du paquet `typescript` ; seul `tsc --noEmit` (OK avec 7) et le lint sont concernés.
*Correction :* remplacer « repli 5.9.x » par « repli 6.0.3 » dans la table ; puis choisir et écrire l'une des deux options : (i) v1 sur **TypeScript 6.0.3** seul (projet petit, le gain 10× de TS 7 est sans objet) et TS 7 en Deferred ; ou (ii) TS 7 pour `tsc --noEmit` **et** `"typescript6": "npm:@typescript/typescript6@6.0.2"` (ou `typescript@6.0.3` aliasé) pour `typescript-eslint`, avec la recette d'alias notée dans la spine. Dans les deux cas, écrire que le `tsconfig` suit les défauts 6.0 (pas de `baseUrl`, `rootDir` explicite). Enfin trancher si `eslint` fait partie de la CI : AD-10 ne le liste pas alors que la table « Stack » le versionne.

**F2 — `scratch` + `user: 10001` + volume nommé `/data` : le point de montage appartient à root, le binaire ne peut pas créer `app.db`.**
Un volume nommé neuf est peuplé depuis l'image : « if files or directories exist, these files or directories are propagated (copied) into the volume by default » (docs.docker.com, volumes), propriétaire compris. Si l'image `scratch` ne contient pas de `/data`, Docker crée le point de montage en `root:root 755` et le processus `10001:10001` reçoit `EACCES` à l'ouverture de `/data/app.db` (et de `/data/backups/`, `/data/reference/`) : le conteneur meurt au premier démarrage, `read_only: true` interdisant tout `chown` de secours, et il n'y a ni shell ni `chown` dans l'image. AD-8 énonce la combinaison sans ce détail, qui est le défaut réel de l'outil.
*Correction :* dans AD-8 (ou AD-1, Dockerfile), ajouter : l'étape finale copie un répertoire vide avec `COPY --chown=10001:10001 --from=builder /empty /data` (et `/data/backups`, `/data/reference`) pour que le volume hérite du propriétaire à sa création ; `check-hardening.sh` vérifie la présence de ce `--chown`. Variante acceptable : `volumes:` avec un volume pré-créé et `chown` par une tâche ponctuelle du runner, mais cela contredit « compose durci seul ».

### Moyenne

**F3 — sqlx 0.9.0 : seule release de la série depuis six mois, correctifs fusionnés non publiés, régressions SQLite ouvertes, SQLite embarquée figée à 3.51.3.**
Faits constatés sur `transact-rs/sqlx` : la version de `main` est toujours 0.9.0 et l'issue #4495 (2026-11-01, « New release with fix from 4394 (and any other fixes since 0.9.0)? ») demande une publication ; #4394 est fusionnée depuis 2026-09-09 sans release. Issues SQLite ouvertes depuis 0.9.0 : **#4322** `INSERT … RETURNING` sur `INTEGER PRIMARY KEY` inféré nullable par `query!` (régression de 0.8.6 → 0.9.0, bissectée à #4088) ; **#4373** (fermée, correctif sur `main` donc absent de 0.9.0) `query!()` fait segfaulter rustc quand une colonne n'a pas de type déclaré ; **#4327** `SqliteConnectOptions` `:memory:` panique dans `to_url_lossy` (la spine teste sur `sqlite::memory:`) ; **#4434** `SQLITE_FULL` laisse une profondeur de transaction fausse et casse les `begin` suivants (disque NAS plein) ; **#4445** un `begin()` imbriqué échoué annule la transaction englobante ; **#4415** exécution multi-statement quadratique (sans effet pour des migrations courtes). Enfin `sqlx-sqlite` 0.9.0 borne `libsqlite3-sys >=0.30.1, <0.38.0` : la seule 0.37.x est 0.37.0 (2026-03-15) qui embarque **SQLite 3.51.3**, alors que `libsqlite3-sys` 0.39.1 embarque 3.54.1 ; la borne ne se contourne pas en aval (#4379). La version est donc exacte mais la spine ne mentionne aucun de ces défauts.
*Correction :* dans AD-2, (i) épingler `sqlx = "=0.9.0"` et prévoir la montée en 0.9.1 comme story explicite ; (ii) décider de l'usage des macros de compilation : préférer `query_as::<_, T>` à l'exécution sur SQLite tant que #4322/#4373 ne sont pas publiées, ou documenter le contournement (`RETURNING id AS "id!"`) ; (iii) noter la version SQLite embarquée (3.51.3) et le fait qu'elle suit sqlx, pas le projet ; (iv) tester explicitement le chemin `SQLITE_FULL` (#4434) dans les tests HTTP, puisque le NAS est le seul environnement.

**F4 — `altcha-lib-rs` 0.3.3 implémente le protocole ALTCHA v1 ; la crate officielle `altcha` 1.1.0 et le widget v3 sont en protocole v2.**
`altcha-lib-rs` (jmic, « Community implementation ») produit des défis `{algorithm, challenge, maxnumber, salt, signature}` (hash itéré + HMAC, `src/lib.rs`). `altcha-org/altcha-lib-rs` publie la crate **`altcha`** 1.1.0 (2026-10-19), « official Rust implementation of the ALTCHA Proof-of-Work (v2) protocol » (PBKDF2/Argon2/scrypt, champs `keyLength`, `keyPrefix`, `expiresAt`, `derivedKey`), avec un `counter_mode: String` « v1 compatibility ». Le widget npm `altcha` est en 3.5.0 et `MIGRATION-v2.md` décrit la v3 comme « a complete rewrite … featuring a redesigned proof-of-work mechanism ». Je n'ai pas pu confirmer que le widget v3 accepte encore un défi v1 (site de doc rendu en JS) : la compatibilité de la crate choisie avec le widget courant est donc **non vérifiée**, et la crate choisie n'est pas celle du projet. Bonne nouvelle pour AD-17 : aucune des deux crates ne tire de client HTTP (`reqwest`, `ureq`, `hyper-util`). Note : `altcha` 1.1.0 dépend de `hmac 0.12` / `sha2 0.10` / `digest 0.10`, qui coexisteront avec les `hmac 0.13` / `sha2 0.11` de la spine (duplication, pas conflit) ; `altcha-lib-rs` 0.3.3 est au contraire aligné (`hmac ~0.13`, `rand ^0.10`).
*Correction :* dans AD-7 et la table « Stack », remplacer `altcha-lib-rs 0.3.3` par **`altcha 1.1.0` (crate officielle, protocole v2)** et accepter la duplication `hmac`/`sha2` ; ou garder la crate communautaire en écrivant que la v1 du protocole est visée et que le solveur côté client sera écrit à la main (WebCrypto), puisque AD-19 interdit toute dépendance front hors React/react-router/Tailwind et que le widget `altcha` npm serait une dépendance nouvelle. Dans tous les cas, lever l'incohérence AD-7 / AD-19 : « preuve de travail sans script tiers » suppose soit le widget embarqué (dépendance front à réviser), soit un solveur maison.

**F5 — `governor` à clé IP : l'état grossit sans borne tant que personne n'appelle `retain_recent()`.**
Documentation de `governor` (`state/keyed.rs`) : « As the inputs to a keyed rate-limiter can be arbitrary keys, the set of retained keys … grows … To save on space, a keyed rate-limiter allows removing those keys that are "stale" » via `retain_recent()` et `shrink_to_fit()`. AD-7 décrit « un limiteur en mémoire … par clé IP » (IPv4 + /64) et se félicite qu'aucune IP ne soit persistée, mais ne prévoit aucun ménage : un scan de masse sur l'hôte public fait croître la `DashMap` jusqu'au `mem_limit: 256m`, et l'OOM-kill est un déni de service du conteneur. C'est le défaut réel de l'outil, pas un bug.
*Correction :* ajouter à AD-7 (ou AD-14) une tâche tokio périodique (`[ASSUMPTION]` toutes les 60 s) qui appelle `retain_recent()` puis `shrink_to_fit()` sur chaque limiteur, et un plafond de clés (`len()`) au-delà duquel le limiteur passe en mode « tout refuser » avec `Retry-After`, journalisé sans IP (AD-15).

### Basse

**F6 — `rust-embed` : dépôt parti de GitHub, forge personnelle, un seul mainteneur.**
`repository` pointe sur `https://pyrossh.dev/repos/rust-embed` (clone `git.pyrossh.dev`), `github.com/pyrossh/rust-embed` renvoie 404 : plus d'issues ni de PR publiques sur GitHub. Les releases continuent (8.13.0 le 2026-10-07) et la crate est stable, donc le choix reste valable ; mais pour un dépôt public durci, c'est un signal de bus factor à noter. La spine sert les assets à la main (repli `index.html`, 404), donc le remplacement serait trivial.
*Correction :* une ligne dans AD-1 : « `rust-embed` (forge personnelle du mainteneur, hors GitHub) ; alternative sans changement de contrat : `include_dir` ». Rien d'autre.

**F7 — `dotenvy` : aucune release depuis 0.15.7 (2023-03-22).**
Dépôt vivant (push 2026-08-18, 49 issues ouvertes) mais rien de publié en trois ans et demi. Usage dev seulement (conventions « Config »), donc sans risque en production.
*Correction :* soit garder en l'écrivant tel quel, soit l'éviter : `[env]` de `.cargo/config.toml` ou le Vite dev server suffisent sur le poste de développement ; la spine n'a alors plus de crate « dev only » à justifier dans `Cargo.lock`.

**F8 — `governor` tire par défaut `rand 0.9` + `getrandom 0.3` (feature `jitter`), en doublon de `rand 0.10` / `getrandom 0.4`.**
`governor` 0.10.4 : `default = ["std", "dashmap", "jitter", "quanta"]`, `jitter = ["dep:rand", "dep:getrandom"]` avec `rand ^0.9`, `getrandom ^0.3`. Deux générateurs dans l'arbre, deux `getrandom` à auditer, pour un jitter que le middleware n'utilise pas.
*Correction :* `governor = { version = "0.10.4", default-features = false, features = ["std", "dashmap", "quanta"] }` noté dans AD-7.

**F9 — Caddy « 2.11.9 » est la release du jour ; l'écrire comme plancher, pas comme version constatée sur les hôtes.**
v2.11.9 est sortie le 2026-11-05 et corrige une régression de 2.11.8 plus des failles (FastCGI) ; le dépôt apt `caddy/stable` la sert déjà. Le VPS et le NAS ne l'ont pas forcément encore ; et aucune directive utilisée par AD-9 (`handle`, `abort`, `remote_ip`, `reverse_proxy`, `header_up`) n'est nouvelle.
*Correction :* « Caddy 2.11.x, ≥ 2.11.9 (apt stable, mise à jour par l'hôte) ».

**F10 — Détails `scratch` à écrire pour qu'ils ne se perdent pas : répertoire temporaire SQLite et mode du tmpfs ; et « mode bibliothèque » de react-router.**
(i) SQLite cherche son répertoire temporaire via `SQLITE_TMPDIR`, puis `TMPDIR`, puis `/var/tmp`, `/usr/tmp`, `/tmp` : `tmpfs: [/tmp]` d'AD-8 couvre `VACUUM INTO` et les tris, à condition que le tmpfs soit inscriptible par uid 10001 (mode par défaut 1777 chez Docker ; poser `SQLITE_TMPDIR=/tmp` dans le compose lève le doute). (ii) react-router v8 nomme « Declarative mode » ce que la spine appelle « mode bibliothèque » ; `react-router-dom` n'existe plus en v8 (bloqué en 7.18.5) et la spine a raison de ne citer que `react-router` ; v8 est ESM-only et exige Node ≥ 22.22 et React ≥ 19.2.7, satisfaits par Node 24 et React 19.3.1.
*Correction :* ajouter `SQLITE_TMPDIR=/tmp` à l'environnement du service dans AD-8 ; écrire « react-router 8.5.1 (Declarative mode, sans `@react-router/dev`) » dans la table.

## Ce qui a été vérifié et n'appelle aucune correction

- Compatibilités croisées : `axum 0.8.9` ↔ `tower-http 0.7.1` (même `http 1` / `tower 0.5`) ; `hmac 0.13.0` ↔ `sha2 0.11.1` (même `digest 0.11`) ; `sqlx-sqlite 0.9.0` ↔ `chrono 0.4.45` ; `tokio 1.54.1` ≥ `^1.44` (axum) ; MSRV sqlx 1.94 ≤ Rust 1.99.1 ; édition 2024 ≥ 1.85.
- Compatibilités front : `@vitejs/plugin-react 6.2.1` → `vite ^8` ; `vitest 5.1.0` → `vite ^6.4 || ^7 || ^8` ; `@tailwindcss/vite 4.3.4` → jusqu'à `vite ^8` ; `react-router 8.5.1` → `react >= 19.2.7`, `@types/react >= 19.2` (19.3.0) ; `typescript-eslint 8.72.1` → `eslint ^10` ; engines Node (`vite` ≥ 22.12, `vitest` ^24, `eslint` ≥ 24, `react-router` ≥ 22.22) satisfaits par Node 24.22.2.
- AD-17 : ni `altcha-lib-rs` ni `altcha` ni `governor` ni `rust-embed` ne tirent `reqwest`, `ureq`, `lettre`, `hickory` ou un client `hyper-util` (les `reqwest`/`hyper-util` d'axum sont optionnels ou en dev-dependencies).
- Compose : `${VAR:?}` à message vide valide ; `mem_limit`/`cpus`/`pids_limit` au niveau service valides et appliqués hors Swarm ; `version` obsolète sans erreur ; Docker Engine 29.9.3 et Compose 5.7.0 courants.

## Décompte

10 findings : 0 critique, 2 hautes (F1, F2), 3 moyennes (F3, F4, F5), 5 basses (F6 à F10).
