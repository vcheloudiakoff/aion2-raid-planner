---
title: 'Coquille SPA bilingue et contrat d''erreurs'
type: 'feature'
created: '2026-11-06'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '7efe0664227bcfdd500f4e46789988b87a777aff'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** La SPA est une page vide et les erreurs de l'API sont des statuts nus : aucun écran ne peut s'appuyer sur une langue, un thème, un client HTTP ni une forme d'erreur commune.

**Approach:** Registre Rust `http::errors::Code` rendu en `application/problem+json` et sous-commande `list-error-codes` vérifiée contre les locales ; côté web, `client.ts` (conditionnel manuel, `X-App-Build`), `t()` et locales fr/en, thème, en-tête et feuille « Langue · Thème », accueil vide rendu depuis la référence en cache, bandeaux hors ligne et nouvelle version (SPEC CAP-7, CAP-10 ; AD-5, AD-12, AD-19).

## Boundaries & Constraints

**Always:** visuel selon les tokens de `DESIGN.md` (sombre par défaut, `prefers-color-scheme` tant que rien n'est choisi), comportements selon `EXPERIENCE.md` (State Patterns, Voice and Tone, Accessibility) ; textes uniquement dans `web/src/locales/{fr,en}.json`, plats, mêmes clés ; tout accès `localStorage` enveloppé (exception = valeur absente, la page marche) ; premier rendu sans attendre l'API ; dépendances d'exécution limitées à react, react-dom, react-router 8.5.1 et Tailwind 4.3.4 (`@tailwindcss/vite`), versions exactes ; budget JS ≤ 150 ko et CSS ≤ 20 ko gzip ; chaque réponse d'erreur `/api/*` = problem+json `{type, title, status, code}` sans phrase destinée à l'utilisateur (`title` = libellé HTTP standard), 503 avec `Retry-After`.

**Never:** aucun bouton « Créer un raid » visible avant l'epic 2 ; aucune bibliothèque d'i18n, d'état, de requêtes ni de composants ; aucune police, image ou script tiers ; pas de rechargement forcé sur nouveau build ; pas de `links.ts` ni de section « Sur cet appareil » remplie (1-6) ; aucun code `auth.*` ni `limit.*` (1-6, 1-7) ; libellés de référence jamais dans les locales.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Référence absente côté serveur | `GET /api/reference`, aucun jeu chargé | 503 problem+json `reference.unavailable`, `Retry-After` | N/A |
| Erreur interne | sérialisation impossible | 500 problem+json sans détail | cause dans `tracing` seulement |
| `list-error-codes` | — | un code par ligne, trié, code 0 | N/A |
| Locale incomplète | `errors.<code>` absent de `fr.json` ou `en.json`, ou clé `errors.*` orpheline | contrôle de CI rouge, code nommé | N/A |
| Premier visiteur | aucun cache, réseau OK | accueil rendu tout de suite, référence chargée puis mise en cache avec son ETag | N/A |
| Visiteur revenu | cache `localStorage` présent | rendu depuis le cache, `If-None-Match` envoyé, 304 → corps du cache | N/A |
| Serveur injoignable | `fetch` rejeté | bandeau « Hors ligne · dernières données de HH:MM » (heure du cache), nouvel essai sur `online` et au retour de visibilité | sans cache : bandeau sans heure |
| Nouveau build | `X-App-Build` différent du premier vu | bandeau « Nouvelle version · Recharger », bouton qui recharge | N/A |
| Stockage indisponible | `localStorage` lève à la lecture ou l'écriture | page fonctionnelle, choix non mémorisés | N/A |
| Langue | `navigator.language` `en-US`, rien choisi | interface en anglais, `<html lang="en">` | langue inconnue → `fr` |
| Feuille | ouverture, Échap, Fermer, glissement | focus piégé puis rendu au déclencheur, `aria-modal`, choix fr/en et sombre/clair mémorisés | `prefers-reduced-motion` : sans transition |

**Décisions (Vincent, 2026-11-06) :**
- Entité `server` ajoutée au registre : `server.not_found` (404, `/api/*` inconnue), `server.method_not_allowed` (405), `server.internal` (500), `server.unavailable` (503, `/api/health` en échec). La liste fermée d'AD-5 sera amendée dans la spine par `bmad-architecture`, pas à la main.
- Spec gardée entière malgré sa taille.

</frozen-after-approval>

## Code Map

- `crates/server/src/http/mod.rs` -- `router_with` : `api_not_found` (404 nu) et `get_health` (503 nu) à convertir ; ajouter `pub mod errors`.
- `crates/server/src/http/reference.rs` -- 503 et 500 nus à remplacer par `Code` ; ne pas toucher à l'ETag ni à `if_none_match`.
- `crates/server/src/http/headers.rs` -- `security_headers` pose déjà `X-App-Build` et `no-store` sur `/api/*` : les réponses d'erreur passent par lui.
- `crates/server/src/main.rs` -- dispatch des sous-commandes et `usage()` : ajouter `list-error-codes` (sans `Config`).
- `crates/server/tests/http.rs`, `tests/cli.rs` -- tests HTTP sur `sqlite::memory:` et tests processus à étendre.
- `.github/workflows/deploy.yml` -- job `ci` : ajouter le contrôle `list-error-codes` contre les locales après `validate-reference`.
- `web/` -- `main.tsx` rend `<App/>` vide ; `index.html` en `lang="fr"` ; `vite.config.ts` (plugin react seul) ; `src/api/types.ts` miroir des DTO de référence (`ReferenceSet`, `Label {en, fr?}`) ; `scripts/check-budget.mjs` ; vitest 5.1.0 sans environnement DOM ; ni Tailwind, ni router, ni locales.
- UX : tokens `DESIGN.md` L12-40, typographie L41-71, rayons et espacements L72-86, feuille L125-127 et L236-249, thème L141-149 ; `EXPERIENCE.md` en-tête L55-56, Voice and Tone L58-79, feuille L93, State Patterns L98-118, accessibilité L132-147, 320-359 px L321-328. Aucune chaîne anglaise n'y figure.

## Tasks & Acceptance

**Execution:**
- [x] `crates/server/src/http/errors.rs` -- enum `Code` exhaustif (`as_str`, `status`, `ALL`), `Problem` qui implémente `IntoResponse` (problem+json, `Retry-After` pour 503) ; tests de forme et d'unicité -- AD-5.
- [x] `crates/server/src/http/{mod,reference}.rs`, `main.rs`, `tests/` -- erreurs existantes via `Code` ; sous-commande `list-error-codes` ; tests HTTP du corps problem+json et de `Retry-After`, test CLI -- matrice.
- [x] `web/scripts/check-error-codes.mjs` (+ test vitest), `.github/workflows/deploy.yml` -- lit les codes sur stdin, échoue si `errors.<code>` manque dans une locale ou si une clé `errors.*` n'a pas de code ; étape `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- CAP-7.
- [x] `web/package.json`, `vite.config.ts`, `src/index.css` -- `react-router` 8.5.1 et `tailwindcss` + `@tailwindcss/vite` 4.3.4 en dépendances exactes, environnement DOM de test en devDependency exacte ; tokens en variables CSS sombre et clair, thème par `data-theme` sur `<html>` -- DESIGN.md.
- [x] `web/src/lib/storage.ts`, `src/i18n/` (`t()`, `fr.json`, `en.json`, langue), `src/theme.ts` -- stockage enveloppé ; langue `navigator.language` puis choix mémorisé, `lang` du document ; thème `prefers-color-scheme` puis choix mémorisé -- AD-12.
- [x] `web/src/api/client.ts`, `src/api/reference.ts`, `src/api/types.ts` -- `getJson` conditionnel (mémoire `etag` + corps par URL, 304 → corps), détection `X-App-Build`, erreur typée `ApiError {status, code?}` ; cache de référence `localStorage` avec ETag et heure ; type `Problem` -- AD-5, AD-19.
- [x] `web/src/App.tsx`, `src/components/{Header,SettingsSheet,Banner}.tsx`, `src/pages/Home.tsx` -- routeur (`/` seul, autre chemin → accueil), en-tête, feuille « Langue · Thème », accueil vide, bandeaux hors ligne et nouvelle version -- CAP-10.
- [x] `web/src/**/*.test.ts(x)` -- locales (mêmes clés, aucune valeur vide), `t()`, `client.ts` (304, `X-App-Build`, rejet réseau), stockage qui lève, langue et thème, focus et Échap de la feuille -- matrice.

**Acceptance Criteria:**
- Given le dépôt, when les commandes du job `ci` tournent en local, then toutes passent, dont le contrôle des codes et le budget.
- Given la page servie à 360 px, when on l'ouvre, then « Aucun raid ouvert pour l'instant. » s'affiche sans défilement horizontal, avec cibles ≥ 44 px et focus visible dans les deux thèmes.

## Design Notes

Textes anglais à écrire selon Voice and Tone (registre familier, sans excuse ni point d'exclamation), par exemple `home.empty` = « No raids open right now. », `banner.newVersion` = « New version · Reload ». Vincent les relit à la revue.

`check-error-codes.mjs` vérifie dans les deux sens : un code sans traduction casse l'interface, une traduction sans code masque une faute de frappe dans le registre.

## Verification

**Commands:**
- `cd web && npm ci && npm run build && npx vitest run && node scripts/check-budget.mjs` -- expected: vert, budget respecté.
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && REQUIRE_WEB_DIST=1 cargo test` -- expected: vert.
- `cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs` -- expected: 0.

**Manual checks (if no CLI):**
- Servir le build (`cargo run` avec un `.env` de test hors dépôt) et ouvrir `/` à 360 px dans les deux thèmes et les deux langues ; couper le serveur → bandeau hors ligne.

## Implementation Notes

- Registre généré par une macro (`codes!`) : enum, `ALL`, `as_str` et `status` viennent d'une seule liste, aucun code ne peut manquer à l'un d'eux. `type` = `about:blank` (RFC 9457), `Retry-After` 60 s par défaut sur 503/429 (`Problem::retry_after` pour 1-7).
- 405 `/api/*` via `method_not_allowed_fallback` ; le 404 de `/api/` (barre finale, repli des assets) passe aussi par `Code`.
- Étape CI en `shell: bash` (pipefail) et liste vide refusée par `check-error-codes.mjs` : un échec de `cargo run` ne passe pas pour un registre vide.
- `client.ts` traite aussi 502/504 de l'edge comme « injoignable » (NAS éteint derrière le VPS).
- Environnement DOM de test : `happy-dom` 21.0.4, par fichier (`@vitest-environment`) ; rendu de test maison (`src/test/dom.ts`), sans bibliothèque de test.
- Nom affiché de l'outil : `app.name` = « aion2-raid-planner » faute de nom produit dans les documents, à confirmer par Vincent.

## Spec Change Log

## Review Triage Log

| # | Source | Constat | Verdict | Preuve | Route |
|---|--------|---------|---------|--------|-------|
| 1 | verif | bouton « Recharger » jamais cliqué en test | medium | `onClick` retiré, tests verts | patch |
| 2 | verif, blind | nouvel essai sur `visibilitychange` non testé | medium | seul `online` est déclenché en test | patch |
| 3 | verif | `fetchedAt` stocké après un 304 non vérifié | low | le test ne relit pas `localStorage` | patch |
| 4 | verif | 504 de l'edge non testé | low | seul 502 testé | patch |
| 5 | verif, blind | bascule du thème système en direct non testée | medium | abonnement au chargement du module, hors de portée des tests | defer |
| 6 | edge, blind | 2xx au corps non JSON → `SyntaxError` brute | low | `res.json()` sans garde ; correction directe | patch |
| 7 | blind | `fetch` sans délai : requête pendue, jamais hors ligne | low | aucun `signal` ; une ligne | patch |
| 8 | edge, blind | cache de référence validé trop largement | low | `data` objet quelconque accepté ; écrans futurs | patch |
| 9 | edge, blind | réponses de `sync()` dans le désordre | low | pas de séquence dans `useReference` | patch |
| 10 | edge, blind | focus non rendu au déclencheur sur Safari | medium | `document.activeElement` = `body` après un clic sur Safari ; le test pré-focalise | patch |
| 11 | edge | Échap inopérant si le focus sort de la feuille | low | écouteur sur la feuille seulement | patch |
| 12 | edge | `pointercancel` ferme la feuille | low | même branche que `pointerup` | patch |
| 13 | blind | 405 `/api/*` sans contrôle d'`Allow` | low | RFC 9110 ; assertion absente | patch |
| 14 | blind | `readLocales` appelé deux fois | low | suppression directe | patch |
| 15 | blind | aucun helper `errors.<code>` → texte, `common.error` inutilisé | low | aucun écran n'affiche d'erreur avant l'epic 2 ; le contrôle de CI tient les clés | rejeté |
| 16 | edge, blind | 503 du serveur sans bandeau | low | backend arrêté = 502 de l'edge, déjà « hors ligne » ; 503 = référence absente, accueil vide exact | rejeté |
| 17 | blind | `Retry-After` ignoré, pas de nouvel essai périodique | low | la matrice fixe `online` et visibilité ; conforme | rejeté |
| 18 | blind | nouveau build entre `index.html` et le premier appel non détecté | low | fenêtre de quelques centaines de ms ; injection du build = nouvelle surface | rejeté |
| 19 | blind | ni `inert` ni blocage du défilement sous la feuille | low | `aria-modal` + piège de focus ; fond couvrant | rejeté |
| 20 | blind | palette claire écrite deux fois dans `index.css` | low | nécessaire avant l'exécution du JS ; dérive bornée à un fichier | rejeté |
| 21 | blind | re-rendu sur changement de langue par convention `useLang()` | low | appliqué partout aujourd'hui | rejeté |
| 22 | blind | Tailwind en `dependencies` | false | AD-19 compte Tailwind parmi les dépendances d'exécution ; la spec l'exige | rejeté |
| 23 | edge | Safari < 14 sans `addEventListener` sur `MediaQueryList` | low | navigateur hors cible | rejeté |
