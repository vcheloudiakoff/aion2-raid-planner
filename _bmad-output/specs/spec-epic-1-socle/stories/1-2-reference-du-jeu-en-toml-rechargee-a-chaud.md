---
title: 'Référence du jeu en TOML, rechargée à chaud'
type: 'feature'
created: '2026-11-06'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '6e55d16bc8ac247c5cea32cd877c124e1e9aa783'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
  - '{project-root}/_bmad-output/specs/spec-epic-1-socle/reference-schema.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Aucune donnée du jeu (régions, serveurs, instances, classes, rôles) n'existe encore ; toutes les écritures des epics 2 à 4 en dépendent, et Vincent doit pouvoir les corriger par simple commit, sans rebuild ni redémarrage.

**Approach:** Cinq fichiers `reference/*.toml` au contenu initial de `reference-schema.md`, un validateur unique (démarrage, tick de rechargement, sous-commande `validate-reference`), un remplacement atomique d'un `Arc` de jeu immuable quand un mtime change, `GET /api/reference` avec ETag, et la santé qui exige une référence chargée (SPEC CAP-3, AD-6).

## Boundaries & Constraints

**Always:** schéma et contrôles exacts de `reference-schema.md` ; contenu initial saisi tel quel, sans serveur de lancement ; slugs anglais ; `label.en` obligatoire, `fr` optionnel ; entrées désactivées servies avec `enabled: false` ; tick ≤ 60 s ; mtime mémorisé seulement après succès ; jeu embarqué dans le binaire utilisé seulement si `/data/reference/` ne contient aucun des cinq fichiers ; même fonction de validation partout ; `/api/reference` reste `no-store` avec `X-App-Build` ; `domain` sans I/O.

**Never:** aucune valeur de référence dans le code Rust ou React (hors tests et fixtures) ; le serveur n'écrit jamais dans `/data/reference/` ; pas de crate de surveillance de fichiers ni d'`arc-swap` ; pas de cache côté client (1-5) ; pas de registre problem+json (1-5) ; pas de `deploy/` ni de CI (1-3).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Lecture | jeu chargé | `GET /api/reference` → 200 JSON camelCase, `ETag` fort | N/A |
| Conditionnel | `If-None-Match` égal (ou `W/` égal, ou `*`) | 304 sans corps, même `ETag` | N/A |
| Rechargement | un fichier modifié et valide | jeu remplacé au tick suivant, nouvel `ETag` | N/A |
| TOML invalide | syntaxe, id dupliqué, `en` manquant, champ inconnu, borne, référence croisée | ancien jeu conservé | `error` en `tracing` (fichier, entrée, cause), une fois par état ; nouvel essai à chaque tick |
| Dossier vide | aucun des cinq fichiers | jeu embarqué | N/A |
| Dossier partiel | 1 à 4 fichiers présents | traité comme invalide | idem TOML invalide |
| Démarrage invalide | aucun jeu valide disponible | serveur démarre, `/api/health` et `/api/reference` → 503 sans corps | réessai à chaque tick |
| CLI | `validate-reference [DIR]` (défaut `/data/reference`) | code 0 et résumé sur stdout / code 1 et chaque problème sur stderr ; dossier vide = 1 | pas de `Config` requise |

</frozen-after-approval>

## Code Map

- `crates/server/src/app/mod.rs` -- `AppState::new(db, clock, ids)` : ajouter le magasin de référence ; appelé par `main.rs` et `tests/common/mod.rs::state`.
- `crates/server/src/app/health.rs` -- `health()` : ajouter « référence chargée ».
- `crates/server/src/http/mod.rs` -- `router_with` : ajouter `/reference` au sous-routeur `/api` ; `headers::security_headers` pose déjà `no-store` et `X-App-Build` sur `/api/*`.
- `crates/server/src/main.rs` -- dispatch `(args.next(), args.next())` : ajouter `validate-reference` avant `Config` ; `run()` : chargement initial puis tâche de tick.
- `Cargo.toml` -- `toml` 1.2.0 et `sha2` 0.11.1 (table Stack), `serde_json` 1.0.154 (déjà en dev-dep de `domain`).
- `crates/server/tests/common/mod.rs` -- `state()`, `TempDir`, `send()` à réutiliser ; `send` à étendre pour les en-têtes de requête.
- Pas de `web/src/api/` encore : créer `types.ts` (AD-5, même commit que le DTO).

## Tasks & Acceptance

**Execution:**
- [x] `reference/{regions,servers,instances,classes,roles}.toml` -- contenu initial, commentaire d'en-tête qui décrit le format (`[[region]]`, `[factions.elyos]`, `room_label_charset`…) -- seule documentation d'édition pour Vincent.
- [x] `crates/domain/src/reference.rs` -- types `ReferenceSet` et entrées (serde, `deny_unknown_fields`), `validate(&ReferenceSet) -> Result<(), Vec<ReferenceIssue>>` -- règles pures, partagées.
- [x] `crates/server/src/infra/reference.rs` -- lecture des cinq fichiers, empreinte (mtime + taille, lue avant le contenu), repli embarqué (`include_str!`), `ReferenceStore` (`RwLock<Option<Arc<ReferenceSet>>>`), `Loader::poll()` -- I/O seulement.
- [x] `crates/server/src/app/tasks/reload.rs` -- boucle de tick (10 s, `spawn_blocking`) -- AD-6.
- [x] `crates/server/src/http/{dto,reference}.rs` -- DTO `ReferenceSet` camelCase (`label.fr` absent si non saisi), handler avec ETag = SHA-256 tronqué du corps, `If-None-Match` -- AD-5.
- [x] `crates/server/src/{app/health.rs,app/mod.rs,main.rs}` -- santé, état, sous-commande et tâche.
- [x] `web/src/api/types.ts` -- miroir TS du DTO.
- [x] `crates/server/tests/reference.rs`, tests unitaires `domain` -- chaque ligne de la matrice, chaque contrôle de `reference-schema.md`, jeu embarqué valide et conforme au contenu initial, sous-commande au niveau processus (`CARGO_BIN_EXE_server`).

**Acceptance Criteria:**
- Given un serveur lancé sur un dossier temporaire, when un libellé est modifié dans un TOML puis un poll a lieu, then `GET /api/reference` renvoie le nouveau libellé et un nouvel `ETag` sans redémarrage.
- Given le dépôt, when `validate-reference reference`, then code 0.
- Given les contrôles 1-1 (`fmt`, `clippy -D warnings`, `test`, `cargo tree` sans crate client réseau), then verts.

## Design Notes

Choix de l'agent : démarrage sans jeu valide = serveur en 503 qui réessaie, plutôt qu'une sortie (pas de boucle de redémarrage, échec visible au health-check du déploiement) ; ids `[a-z0-9_-]{1,32}` ; `enabled` facultatif, `true` par défaut ; champ inconnu refusé (attrape une faute de frappe sur `enabled`).

La sérialisation JSON et l'ETag sont calculés dans `http` à chaque requête (≈ 5 ko) : les mettre en cache dans `infra` ferait dépendre `infra` du DTO. L'empreinte est prise avant la lecture : une écriture concurrente change le mtime et déclenche un nouveau chargement au tick suivant ; une copie non atomique de `docker compose cp` (fichiers croisés incohérents) échoue en validation et se résout au tick suivant.

```toml
# servers.toml
[factions.elyos]
label = { en = "Elyos", fr = "Élyséens" }

[[server]]
id = "siel"
region = "europe"
faction = "elyos"
tag = "Siel"
label = { en = "Siel" }
```

## Verification

**Commands:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` -- expected: vert.
- `cargo run -q -p server -- validate-reference reference` -- expected: code 0.
- `cd web && npx tsc --noEmit` -- expected: vert.

## Implementation Notes

- Format TOML : `[[region]]`, `[factions.elyos]` / `[factions.asmodian]` + `[[server]]`, `room_label_charset` / `room_label_max` + `[[instance]]`, `[[class]]`, `[[role]]` ; chaque fichier documente son format en commentaire d'en-tête.
- `[ASSUMPTION]` à vérifier en jeu : `room_label_charset` = `A–Z0–9`, `room_label_max = 8`, `room_label` de Ludra = `LUDRA`. Le libellé d'un rôle est son libellé court (pas de champ séparé).
- Contrôles ajoutés : `en` blanc = manquant, `fr` blanc refusé.
- DTO : `factions` servi en `{ elyos, asmodian }` ; la règle de `room_label` n'est pas envoyée au client.
- Jeu embarqué par `include_str!` depuis `reference/` : le Dockerfile de 1-3 doit inclure ce dossier dans le contexte de build.
- `AppState::new` prend un quatrième argument (`ReferenceStore`) ; aides de test `state_with`, `send_with`.
- Dépendances : `toml` 1.2.0 (features `std`, `serde`, `parse`), `sha2` 0.11.1, `serde_json` 1.0.154 passé en dépendance de workspace, feature `time` de tokio.

## Spec Change Log

## Review Triage Log

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | verif | `faction` et `kind` sérialisés sans assertion JSON | medium | le test HTTP ne lit ni `servers[*].faction` ni `instances[*].kind` ; seuls les types domaine sont vérifiés | patch |
| 2 | verif | câblage de `run()` (store partagé, poll initial, tâche) non testé | medium | aucun test n'exécute `serve` ; il faudrait extraire `run_with(dir, db, listener)` ou un smoke test conteneur | defer |
| 3 | verif, blind, edge | une panique de `poll` arrête le rechargement, santé toujours 200 | low | seule source de panique avancée (découpe de span TOML) réfutée en #5 ; garde = boucle de reprise, complexité | rejeté |
| 4 | blind, edge | empreinte mtime + taille rate une édition de même taille dans la même granularité | low | `git reset` réécrit le fichier, `docker cp` reporte ce mtime neuf ; mtime en ns sur ext4/overlay ; un hash ajoute une lecture par tick | rejeté |
| 5 | edge | découpe `text[..span.start]` hors frontière UTF-8 → panique | false | essais avec `é`, `€`, `×` aux positions d'erreur (valeur, clé, chaîne non fermée) : messages corrects, aucune panique ; les spans du parseur tombent sur des frontières de caractère | rejeté |
| 6 | blind, edge | bascule sur le jeu embarqué si les cinq fichiers disparaissent en cours de route | low | conforme à la règle figée « jeu embarqué si le dossier ne contient aucun des cinq fichiers » ; suppression délibérée seulement | rejeté |
| 7 | blind | renommage ou suppression d'id non contrôlé entre deux jeux | false | AD-6 : une lecture tolère un id absent (repli sur l'id) ; la règle est une consigne d'édition | rejeté |
| 8 | blind, edge | `tag` accepte espaces de bord, contrôles, doublons à la casse près | low | erreur d'édition improbable, visible dans `validate-reference` ; ajouterait des gardes | rejeté |
| 9 | blind | libellés sans longueur max ni filtre de contrôles | low | données éditées par Vincent seul ; ajouterait des gardes | rejeté |
| 10 | blind | `room_label` non unique entre instances | false | aucune règle ne l'exige ; le nom de salle porte aussi un code aléatoire (AD-18) | rejeté |
| 11 | blind | entrée active sous un parent désactivé acceptée | false | AD-6 : « existe et `enabled` » ne porte que sur les champs que l'écriture pose | rejeté |
| 12 | blind | `default_max` sans borne liée à la capacité | low | valeur aberrante d'édition, sans consommateur avant l'epic 4 | rejeté |
| 13 | blind | miroir TS sans test d'accord avec le DTO | false | AD-5 : miroir à la main en v1, même commit | rejeté |
| 14 | blind | Dockerfile, compose et CI absents | false | hors périmètre (1-3), décision de la spec | rejeté |
| 15 | blind | test du chemin par défaut conditionnel à l'absence de `/data/reference` | low | `/data` n'existe pas sur les postes ni en CI | rejeté |
| 16 | blind | calcul ligne/colonne de `parse_file` sans assertion | low | le cas `syntax` n'attend que le nom du fichier ; correction directe du test | patch |
| 17 | blind | journal de chargement réussi sans comptes | low | le chemin d'échec est détaillé ; confort d'exploitation | rejeté |
| 18 | blind | story et sprint-status absents du diff | false | exclus volontairement du diff de revue | rejeté |
| 19 | edge | `spawn` avec période nulle panique | false | seuls appelants : `TICK` (10 s) et un test à 20 ms | rejeté |
| 20 | edge | `modified()` en erreur → seule la taille compte | low | volume Docker local avec mtime ; aucun système de fichiers ciblé sans mtime | rejeté |
| 21 | edge | fichiers manquants masquent les fichiers illisibles dans le rapport | low | combinaison improbable ; le rapport cite déjà les manquants | rejeté |
| 22 | edge | `validate-reference ""` valide le dossier courant | low | argument vide improbable, la CI passe `reference` | rejeté |
| 23 | edge | rechargement indexé sur l'empreinte, pas le contenu | low | même cause que #4 | rejeté |
