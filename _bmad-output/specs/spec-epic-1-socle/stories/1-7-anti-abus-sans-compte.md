---
title: 'Anti-abus sans compte'
type: 'feature'
created: '2026-11-06'
status: 'done'
baseline_revision: '203319d52f6349b14c79ff8e621f691eab2b7ea7'
review_loop_iteration: 0
followup_review_recommended: false
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md'
  - '{project-root}/_bmad-output/specs/spec-epic-1-socle/configuration.md'
warnings: [oversized]
deferred:
  - summary: >-
      Le lancement de la tâche de purge du limiteur par `run` n'est vérifié par aucun test.
    evidence: |-
      Supprimer `tasks::limits::spawn` dans main.rs laisse tous les tests verts ; extraire le démarrage en fonction testable.
    location: >-
      crates/server/src/main.rs
    severity: medium
  - summary: >-
      Un corps JSON mal formé sur un POST de formulaire renvoie le rejet texte d'axum, pas un problem+json.
    evidence: |-
      `Json<FormEnvelope<T>>` sans mappage de `JsonRejection` ; à traiter avec les premières commandes de l'epic 2.
    location: >-
      crates/server/src/http/form_token.rs
    severity: medium
---

<intent-contract>

## Intent

**Problem:** l'outil est public et sans compte. Sans frein, une rafale, un robot ou une création en masse saturent le NAS ou polluent la liste. Le frein doit tenir sans script tiers et sans jamais conserver d'IP (SPEC CAP-9, AD-7).

**Approach:** tout le frein vit dans l'application, en mémoire, sur des routes de test, avant toute route métier :
- **IP visiteur :** lue dans un en-tête de confiance à valeur unique.
- **Limiteur :** `governor`, par classe de route.
- **Jeton de formulaire :** HMAC sans état, émis par `GET /api/form-token`.
- **Honeypot.**
- **Bornes de champs :** pures, dans `domain::validate`.
- **Disjoncteur :** à seuils horaires.

## Boundaries & Constraints

**Always:**
- **Classes de route** (`http::limits::RouteClass`) : `create`, `candidature`, `write`, `read`, et `nokey`.
  - `nokey` couvre `/api/health`, les assets, `index.html` et le repli 404 de `/api`. Elle est exemptée du limiteur et de l'extracteur d'IP.
  - `/api/reference` et `/api/form-token` sont en classe `read`.
- **IP visiteur :**
  - Elle est lue dans l'en-tête nommé par `CLIENT_IP_HEADER`, jamais dans `X-Forwarded-For`.
  - Avec `TRUST_CLIENT_IP_HEADER=true`, un en-tête absent, multi-valué ou non analysable donne 400 `limit.client_ip_missing`, sans repli sur l'adresse du pair.
  - Avec `false`, l'adresse du pair est lue par `ConnectInfo<SocketAddr>`.
  - La clé de limitation est l'adresse IPv4 entière, ou le préfixe /64 en IPv6.
- **Quotas :** 429 `limit.rate_limited` avec `Retry-After` en secondes entières (≥ 1), calculé par `http::limits` depuis l'attente de `governor`. `retain_recent()` est appelé à chaque tick d'une tâche périodique.
- **Jeton de formulaire :**
  - Contenu : `base64url(ts ‖ nonce 16 o ‖ classe ‖ HMAC-SHA256(FORM_TOKEN_SECRET, ts ‖ nonce ‖ classe))`.
  - `ts` en secondes Unix, selon la `Clock` injectée. `classe` ∈ {`create`, `candidature`}.
  - Il est exigé par les POST de classe `create` et `candidature`.
  - Âge accepté : de `MIN_SUBMIT_DELAY_MS` à une heure. Le jeton est rejouable dans cette fenêtre. Aucune table.
- **Corps de formulaire :** les champs `formToken` et `website` (honeypot) voyagent dans le corps JSON, à plat à côté des champs métier.
- **Disjoncteur :**
  - Deux détentes, `events` (`MAX_EVENTS_PER_HOUR`) et `candidatures` (`MAX_CANDIDATURES_PER_HOUR`), comptées sur une heure glissante selon la `Clock` injectée.
  - `check` passe avant l'écriture, `record` après le commit.
  - Seuil atteint : 503 `limit.paused` avec `Retry-After` jusqu'à la sortie de la plus ancienne écriture.
- **Bornes de champs** (`domain::validate`), pures, avec une erreur typée par cas :
  - **pseudo :** 1 à 32 caractères, tous `char::is_alphanumeric` (lettres de toute écriture et chiffres, sans espace, crochet ni ponctuation). `[ASSUMPTION]` à confirmer contre le client du jeu.
  - **note :** ≤ 280 caractères, sans URL (`://`, `www.`).
  - **CP :** entier de 0 à 1 000 000. `[ASSUMPTION]`
  - **`discord_url` :** `https://` puis un hôte exact de l'allowlist `discord.gg`, `discord.com`, `www.discord.com`, sans userinfo ni port, avec un chemin non vide.
- **Codes d'erreur :** le registre et les deux locales reçoivent `limit.client_ip_missing` (400), `limit.rate_limited` (429), `limit.paused` (503), `form.token_invalid` (400 : absent, mal formé, mauvaise classe, MAC faux ou plus d'une heure), `form.too_fast` (400) et `form.rejected` (400 : honeypot rempli).
- **Aucune IP en base ni dans les journaux.** Le type de clé n'a ni `Display` ni `Debug` en clair. Un test capture la sortie `tracing` d'une requête limitée puis refusée et vérifie qu'aucune IP n'y figure.

**Never:**
- Ni interrupteur `creation_enabled`, ni route admin, ni preuve de travail, ni `altcha` (non-goals de l'epic).
- Aucune route métier.
- Aucune route de test compilée dans le binaire de production.
- Aucune table, aucune IP écrite où que ce soit, aucun script tiers.
- Aucune dépendance hors `governor` (sans la feature `jitter`) et `hmac`, aux versions de la table *Stack*.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| IP absente (prod) | `TRUST=true`, route `read` sans en-tête | 400 `limit.client_ip_missing` | — |
| IP multi-valuée | deux en-têtes, ou valeur `a, b` | 400 `limit.client_ip_missing` | — |
| Route nokey | `/api/health` sans en-tête | 200 | — |
| Pair en dev | `TRUST=false`, `ConnectInfo` présent | clé = adresse du pair | — |
| Quota `create` dépassé | quota 2/h, 3e POST de la même IP | 429 `limit.rate_limited`, `Retry-After` ≥ 1 | — |
| IPv6 groupé | `2001:db8::1` puis `2001:db8::2` (même /64) | même compteur | — |
| IPv6 autre /64 | `2001:db8:0:1::1` | compteur distinct | — |
| Jeton valide | émis il y a 5 s, classe `create` | 2xx | — |
| Jeton sans `formToken` | POST `create` | 400 `form.token_invalid` | — |
| Jeton trop jeune | âge < `MIN_SUBMIT_DELAY_MS` | 400 `form.too_fast` | — |
| Jeton expiré | âge > 3600 s | 400 `form.token_invalid` | — |
| Mauvaise classe | jeton `candidature` sur POST `create` | 400 `form.token_invalid` | — |
| MAC altéré | un octet du jeton changé | 400 `form.token_invalid` | — |
| Honeypot | `website` non vide | 400 `form.rejected` | — |
| Disjoncteur | `MAX_EVENTS_PER_HOUR` écritures dans l'heure | 503 `limit.paused`, `Retry-After` | — |
| Fenêtre glissée | horloge avancée au-delà d'une heure | écriture de nouveau acceptée | — |

</intent-contract>

## Code Map

- `crates/server/src/config.rs` -- `Config` porte déjà `client_ip_header`, `trust_client_ip_header`, `min_submit_delay`, `max_*_per_hour`, `rate_*: Rate {count, per}` et `form_token_secret.expose()`. Il n'est pas encore utilisé : `main.rs:194` `run(_config)`. `Config::from_source(closure)` sert à construire une config de test.
- `crates/server/src/app/mod.rs` -- `AppState::new(db, clock, ids, reference)`. Il faut y ajouter la config (ou les réglages utiles) et l'état anti-abus partagé (`Arc`). Appelants : `main.rs:217` et `tests/common/mod.rs` (`state_with`).
- `crates/server/src/infra/clock.rs` -- `Clock::now() -> Timestamp`, à la seconde ; `FixedClock` est réglable pour les tests.
- `crates/server/src/http/mod.rs` -- `router_with`, `serve` (`axum::serve(listener, router)` : passer à `into_make_service_with_connect_info::<SocketAddr>()`).
- `crates/server/src/http/links.rs` -- modèle à suivre : extracteur, routes `#[cfg(test)]` montées dans les tests unitaires du crate (`cfg(test)` ne vaut pas pour `tests/`), rejet en `Code`.
- `crates/server/src/http/errors.rs` -- macro `codes!`, `Problem::retry_after(secs)` pour 429 et 503.
- `crates/server/src/app/tasks/` -- modèle de tâche périodique (`reload.rs`, `backup.rs`) pour le tick `retain_recent()`.
- `crates/domain/src/lib.rs` -- ajouter `pub mod validate`. Le domaine n'a aucune I/O et n'ajoute aucune dépendance.
- `Cargo.toml` (workspace) -- versions épinglées `=x.y.z`. Ajouter `governor = "=0.10.4"` (`default-features = false`, sans `jitter`) et `hmac = "=0.13.0"`.
- `deploy/check-network-crates.sh` -- doit rester vert avec les nouvelles crates.
- `.github/workflows/deploy.yml:166,191` -- le test de fumée fait `curl /api/reference` sans en-tête d'IP, ce qui donnera 400. Ajouter `-H 'X-Client-IP: 127.0.0.1'`.
- `deploy/caddy/vps.Caddyfile` -- `header_up X-Client-IP {remote_host}` écrase déjà la valeur du client.
- `deploy/README.md` -- procédure ; y ajouter la vérification de fin d'epic.
- `web/src/locales/fr.json`, `en.json` -- clés `errors.*`.

## Tasks & Acceptance

**Execution:**
- `Cargo.toml`, `crates/server/Cargo.toml` -- ajouter `governor` et `hmac`.
- `crates/domain/src/validate.rs` (+ `lib.rs`) -- fonctions `pseudo`, `note`, `cp` et `discord_url`, avec l'enum d'erreurs `FieldError`. Tests unitaires : bornes exactes (0/1, 32/33, 280/281, 1 000 000/1 000 001), hangul et lettres accentuées acceptés, `[`, `]`, espace, `https://` dans une note, `discord.com.evil.io`, `user@discord.gg`, `discord.gg:443`, `http://`.
- `crates/server/src/http/client_ip.rs` -- `IpKey` (v4, ou v6 réduit à /64 ; `Debug` caviardé) et sa résolution depuis les en-têtes, la config et `ConnectInfo`.
- `crates/server/src/http/limits.rs` --
  - `RouteClass`, les limiteurs `governor` à clé par classe, construits depuis les `Rate` de la config ;
  - un middleware par classe (`route_layer`), qui résout l'IP puis consomme le quota ;
  - le calcul de `Retry-After` ;
  - le `Breaker` (`check` / `record`) à fenêtre glissante sur la `Clock`.
- `crates/server/src/http/form_token.rs` -- `issue(class)` et `verify(token, class, now, min_delay)`, la route `GET /api/form-token?class=create|candidature` qui renvoie `{ "token": … }` (`class` inconnue = 400 `form.token_invalid`), et `FormEnvelope<T> { formToken, website, #[serde(flatten)] data: T }` avec son contrôle.
- `crates/server/src/http/errors.rs` -- les six codes.
- `crates/server/src/http/mod.rs`, `src/main.rs` -- classes sur les routes ; tâche `retain_recent()` ; `ConnectInfo` ; config passée à l'état.
- `crates/server/src/app/tasks/limits.rs` -- tick périodique qui appelle `retain_recent()` sur chaque limiteur.
- Tests unitaires (`#[cfg(test)]`) -- routes POST de test de classe `create` et `candidature` (jeton, honeypot, `check` et `record` du disjoncteur) et une route `read`. Ils couvrent toute la matrice, plus la capture `tracing` sans IP (les valeurs de test viennent de 192.0.2.0/24 et 2001:db8::/32, adresses de documentation).
- `crates/server/tests/` -- adapter les tests existants qui appellent `/api/reference` sans en-tête d'IP, et `common::state_with`.
- `.github/workflows/deploy.yml` -- en-tête `X-Client-IP: 127.0.0.1` sur les `curl` de `/api/reference`.
- `web/src/locales/fr.json`, `en.json` -- traductions, au ton d'EXPERIENCE.md :
  - `rate_limited` : « Doucement : réessaie dans un moment. » ;
  - `paused` : « Pause : trop de créations en une heure. Réessaie plus tard. » ;
  - `too_fast` : « Un peu vite : relis et renvoie. » ;
  - `token_invalid` : « Le formulaire a expiré. Recharge la page. » ;
  - `rejected` : « Envoi refusé. » ;
  - `client_ip_missing` : « Requête refusée par le serveur. »
- `deploy/README.md` -- section « Vérification de fin d'epic 1 » :
  - le signal de succès du SPEC, pas à pas : accueil en fr et en sur téléphone, `/api/health` vert, libellé de classe changé dans `reference/` et visible sans rebuild, relecture du compose et des en-têtes ;
  - un `curl` sans `X-Client-IP` sur le port local donne 400 ;
  - via le VPS, `/api/reference` répond 200.

**Acceptance Criteria:**
- Given le binaire de production, when on liste ses routes, then seule `GET /api/form-token` est nouvelle : les routes de test n'existent que sous `#[cfg(test)]`.
- Given `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs`, then les 14 codes sont présents dans les deux locales.
- Given le job `ci`, when le test de fumée tourne, then `/api/reference` répond 200 avec l'en-tête d'IP.

## Spec Change Log

## Review Triage Log

Passe 1 : 2 medium, 17 low, 1 false, 0 maybe-false (26 constats, regroupés).

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | verif, blind | `serve` avec `ConnectInfo` jamais testé en `TRUST=false` | medium | tests unitaires injectent `ConnectInfo` à la main ; test réseau réel ajouté, `test_config` à surcharges | patch |
| 2 | verif, blind, edge | `check` puis `record` du disjoncteur non atomiques | low | pool d'écriture à une connexion : appelés dans la transaction, la paire est sérialisée ; règle écrite sur `Breaker` et dans le gabarit | patch |
| 3 | edge | horloge qui recule : entrées futures, `Retry-After` > 1 h | low | `prune` écarte `t > now`, attente bornée à 1..=3600 ; test | patch |
| 4 | blind, edge | `MIN_SUBMIT_DELAY_MS` ≥ 1 h bloque tout formulaire en silence | low | refus au démarrage, erreur sans valeur ; test | patch |
| 5 | blind, edge | `discord_url` sans longueur maximale | low | `DISCORD_URL_MAX_CHARS = 256`, `DiscordTooLong` ; tests 256/257 | patch |
| 6 | blind | refus honeypot et jeton non journalisés | low | `info` avec classe et code seuls ; test sans IP ni jeton étendu | patch |
| 7 | blind | fumée CI ne vérifie que le statut 400 | low | corps vérifié sur `limit.client_ip_missing` | patch |
| 8 | verif | lancement de la tâche de purge par `run` non vérifié | medium | `run` non testable sans extraction, comme `reload` et `backup` | defer |
| 9 | blind | rejet `Json` d'axum hors problem+json dans le gabarit | medium | corps mal formé = rejet texte d'axum ; touche toutes les commandes de l'epic 2 | defer |
| 10 | edge | tâche de purge non supervisée | low | même modèle que `reload` et `backup` ; panique improbable | rejeté |
| 11 | blind | deux horloges (governor et `Clock`), test à `sleep` 100 ms | low | choix noté ; test passé 25 fois | rejeté |
| 12 | blind | journaux sans IP testés sur routeur de test seulement | low | aucune couche de traçage en production | rejeté |
| 13 | blind | pièges de `#[serde(flatten)]` | low | aucun DTO métier avant l'epic 2 | rejeté |
| 14 | blind, edge | `note` : caractères de contrôle, bidi, URL sans schéma | low | conforme à la règle (`://`, `www.`) ; React échappe le rendu | rejeté |
| 15 | blind, edge | pseudo non normalisé NFC, homoglyphes | low | normaliser = nouvelle dépendance interdite ; saisie navigateur en NFC ; signalé à Vincent avec l'`[ASSUMPTION]` | rejeté |
| 16 | blind | le client ne lit pas `Retry-After` | low | aucun écran avant l'epic 2 | rejeté |
| 17 | edge | NAT64 : clients IPv4 dans un même /64 | low | cas improbable pour une légion | rejeté |
| 18 | edge | `verify` sans paramètre `min_delay` | false | délai fixé à la construction depuis la config, même effet | rejeté |

## Design Notes

**Pas de repli silencieux sur l'adresse du pair.** Derrière Caddy, l'adresse du pair est celle du proxy. Un repli ferait partager un seul quota à tous les visiteurs, et un oubli de configuration passerait inaperçu.

**Disjoncteur en mémoire.** Il repart de zéro au redémarrage : c'est voulu, puisque la spine n'a aucune table de compteurs. Les commandes réelles (epic 2) appelleront `check` avant la transaction et `record` après le commit.

**Jeton à la seconde.** La `Clock` est à la seconde, donc l'âge est mesuré en secondes entières : `âge_s * 1000 >= MIN_SUBMIT_DELAY_MS`. L'arrondi peut accepter un envoi jusqu'à une seconde trop tôt, ce qui est sans effet sur un robot qui envoie en quelques millisecondes.

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` -- expected: vert.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: code 0.
- `bash deploy/check-network-crates.sh && bash deploy/tests/check-hardening.test.sh` -- expected: vert.
- `cd web && npx tsc --noEmit && npx vitest run` -- expected: vert.
- `docker compose -f deploy/docker-compose.yml build` puis le test de fumée local (`curl` avec et sans `X-Client-IP`) -- expected: 200 avec, 400 `limit.client_ip_missing` sans.

## Auto Run Result

Status: done

**Résumé :** anti-abus sans compte. IP visiteur par en-tête de confiance (400 sans repli), limiteur `governor` par classe avec `Retry-After`, purge périodique, jeton HMAC sans état (`GET /api/form-token`), honeypot, disjoncteur horaire à deux détentes, bornes `domain::validate`, six codes traduits, procédure de fin d'epic dans `deploy/README.md` §14.

**Fichiers :**
- `crates/domain/src/validate.rs` -- pseudo, note, CP, `discord_url`.
- `crates/server/src/http/{client_ip,limits,form_token}.rs` -- IP, limiteur, disjoncteur, jeton.
- `crates/server/src/http/abuse_tests.rs` -- routes de test `cfg(test)` et matrice.
- `crates/server/src/app/tasks/limits.rs` -- `retain_recent()` périodique.
- `crates/server/src/{config,main}.rs`, `app/mod.rs`, `http/{mod,errors,dto}.rs`, `infra/ids.rs` -- câblage, codes, `nonce()`.
- `crates/server/tests/*` -- en-tête d'IP par défaut, test `serve` réel en `TRUST=false`.
- `.github/workflows/deploy.yml`, `deploy/README.md`, locales, `web/src/api/types.ts`.

**Revue :** 7 patchs (1 medium, 6 low), 2 différés, 9 rejetés.

**Relecture complémentaire recommandée :** non (aucun `high`, un seul `medium` patché, couvert par son test).

**Vérification :** fmt, clippy `-D warnings`, `cargo test` verts ; 14 codes dans 2 locales ; contrôle des crates réseau ; `check-hardening` 86/86 ; tsc et vitest 80/80 ; image construite et lancée : 400 `limit.client_ip_missing` sans en-tête, 200 avec.

**Risques résiduels :**
- Bornes `[ASSUMPTION]` à confirmer par Vincent : pseudo de 1 à 32 lettres ou chiffres, CP de 0 à 1 000 000.
- Fin d'epic : le déploiement vérifié contre le signal de succès (§14 du README) reste un geste de Vincent, après le premier déploiement de la story 1-4.
