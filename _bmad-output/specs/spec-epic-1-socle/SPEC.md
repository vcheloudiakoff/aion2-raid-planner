---
id: SPEC-epic-1-socle
companions:
  - reference-schema.md
  - configuration.md
  - ../../planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md
sources:
  - ../../forge/aion2-raid-sans-compte/forged-idea.md
  - ../../../docs/intent-dump.md
  - ../../../docs/domaine-aion2.md
---

> **Contrat canonique.** Ce SPEC et les fichiers de `companions:` forment le contrat complet de ce qu'il
> faut construire, tester et valider. Les `AD-n` renvoient à `ARCHITECTURE-SPINE.md`, qui fait foi et ne
> se modifie pas ici.

# Epic 1 — Socle

## Why

Mandat et douleur à la fois : l'outil est public et tourne sur le NAS personnel de Vincent, il ne doit
jamais devenir un tremplin vers ce NAS ni révéler l'adresse du domicile. Avant tout écran métier, il faut
le binaire unique, le conteneur durci, le déploiement par push, la référence du jeu éditable et les deux
mécanismes dont dépend tout le reste sans compte : les droits par liens secrets et l'anti-abus. Les
epics 2 à 5 ne font que poser des routes et des écrans sur ce socle. L'utilisateur de cet epic est
Vincent, en exploitant.

## Capabilities

- **CAP-1**
  - **intent:** Un seul binaire sert l'API sous `/api/*` et la SPA embarquée, et expose son état de santé (AD-1, AD-8).
  - **success:** `GET /api/health` renvoie 200 quand la base est ouverte, les migrations à jour et la référence chargée, sinon 503 ; la sous-commande `healthcheck` appelle la même `app::health()` et sort en code ≠ 0 sur échec ; toute route GET sans extension renvoie `index.html`, un asset absent renvoie 404 ; `cargo check` passe sans build web (`web/dist/.gitkeep`).
- **CAP-2**
  - **intent:** La persistance repose sur un fichier SQLite unique et des types de temps et d'identifiants communs à tous les epics (AD-2, AD-11, AD-16).
  - **success:** test HTTP sur `sqlite::memory:` : pragmas posés, pool d'écriture à 1 connexion, migrations appliquées au démarrage ; `domain::Timestamp` n'accepte et n'émet que `YYYY-MM-DDTHH:MM:SSZ` ; `Clock` et le générateur d'ids et de secrets sont injectables ; aucun `datetime('now')` ni `CURRENT_TIMESTAMP` dans le dépôt (grep en CI).
- **CAP-3**
  - **intent:** Vincent met à jour régions, serveurs, instances, classes et rôles en éditant des TOML, sans rebuild ni redémarrage (AD-6, Q8).
  - **success:** un commit qui ne touche que `reference/` est visible dans `GET /api/reference` en moins de deux minutes après le push, sans `docker compose build` ; un TOML invalide laisse l'ancien jeu en service et journalise l'erreur ; `validate-reference` échoue en CI sur un id dupliqué, un `en` manquant ou un `room_label` hors charset ; schéma dans `reference-schema.md`.
- **CAP-4**
  - **intent:** L'application tourne dans un conteneur qui ne donne aucune prise sur le NAS, et chaque push est vérifié avant d'y arriver (AD-8, AD-10, AD-17, AD-19).
  - **success:** `deploy/check-hardening.sh` passe sur le compose livré et échoue sur chacun des motifs interdits listés par AD-10 (test par fixtures) ; le job `ci` exécute toute la liste d'AD-10, y compris l'absence de crate client réseau et le budget de poids ; l'image finale est `FROM scratch` et tourne en `10001:10001`.
- **CAP-5**
  - **intent:** Un push sur `main` déploie en production sur le NAS sans geste manuel, avec sauvegarde et retour arrière connus (AD-2, AD-10).
  - **success:** un push vert déploie, le health-check du job passe ; un push qui modifie ou supprime une migration existante fait échouer le job avant tout remplacement de conteneur ; un push qui ajoute une migration produit un `VACUUM INTO` préalable ; la tâche périodique garde deux sauvegardes quotidiennes ; la procédure de restauration écrite dans `deploy/` a été jouée une fois.
- **CAP-6**
  - **intent:** Le site est joint par l'edge existant (VPS, tunnel WireGuard), et l'application pose elle-même ses en-têtes de sécurité (AD-9, Q5).
  - **success:** `deploy/caddy/` contient les blocs VPS et NAS d'exemple avec placeholders ; une réponse de production porte la CSP exacte d'AD-9, `Referrer-Policy: no-referrer`, `nosniff`, `X-App-Build`, `Cache-Control` différencié assets / API ; le bloc NAS refuse toute IP autre que celle du tunnel.
- **CAP-7**
  - **intent:** Toutes les erreurs de l'API ont une forme unique et chaque code a sa traduction (AD-5, AD-12).
  - **success:** toute erreur est `application/problem+json` avec `code = <entité>.<cas>` issu de l'enum `http::errors::Code` ; `list-error-codes` alimente un test de CI qui échoue si `errors.<code>` manque dans `fr.json` ou `en.json` ; aucun texte destiné à l'utilisateur n'est émis par le serveur.
- **CAP-8**
  - **intent:** Les droits d'organisateur, de candidature directe, de participant et d'administration sont portés par des secrets de lien vérifiables sans jamais être stockés en clair (AD-3, Q2).
  - **success:** tests HTTP sur routes de test : secret absent = 401 `auth.missing`, inconnu = 404, d'un autre droit ou d'une autre portée = 403 `auth.wrong_right` ; la base ne contient que `SHA-256(secret)` ; un secret en query string ou en cookie est ignoré ; `links.ts` lit et écrit `links.v1` et la page reste fonctionnelle quand `localStorage` lève une exception (vitest).
- **CAP-9**
  - **intent:** Un abus (rafale, robot, création en masse) est freiné sans compte, sans script tiers et sans conserver d'IP (AD-7, Q1).
  - **success:** tests HTTP : la classe `create` dépasse son quota → 429 avec `Retry-After` ; en `TRUST_CLIENT_IP_HEADER=true`, en-tête absent ou multi-valué → 400 `limit.client_ip_missing` ; un POST de classe `create` ou `candidature` sans jeton, avec un jeton de moins de `MIN_SUBMIT_DELAY_MS` ou de plus d'une heure, ou avec le honeypot rempli, est refusé ; le seuil `MAX_EVENTS_PER_HOUR` atteint → 503 ; IPv6 groupé par /64 ; aucune IP dans la base ni dans les journaux.
- **CAP-10**
  - **intent:** Un visiteur qui ouvre l'URL publique voit une coquille rapide, bilingue et à son thème, qui sait se recharger après un déploiement et survivre au hors-ligne (AD-5, AD-12, AD-19, EXPERIENCE *State Patterns*).
  - **success:** à 360 px, l'accueil affiche « Aucun raid ouvert pour l'instant. » sans défilement horizontal ; le premier rendu n'attend pas l'API (référence en cache `localStorage` avec ETag) ; la feuille « Langue · Thème » bascule fr/en et sombre/clair et mémorise le choix ; un changement de `X-App-Build` affiche « Nouvelle version · Recharger » ; serveur injoignable → bandeau « Hors ligne » ; `lang` suit la langue ; les locales fr et en ont les mêmes clés (test).

## Constraints

- Isolation du conteneur non négociable : non-root, `read_only`, `cap_drop: ALL`, `no-new-privileges`, limites mémoire et CPU, port lié à `127.0.0.1` seulement ; jamais `privileged`, `pid: host`, socket Docker ni bind mount (AD-8).
- Dépôt public : aucun secret, IP, nom d'hôte ni donnée de joueur, y compris dans `deploy/`, les workflows et les exemples Caddy ; `.env` sur le NAS seulement, `deploy/.env.example` sans valeur secrète.
- Le conteneur n'initie aucune connexion sortante : aucune crate client HTTP, SMTP ou DNS (AD-17).
- Aucun secret de lien en clair en base, en URL d'API, en cookie ou en journal ; aucune IP en base ni en journal en production (AD-3, AD-7, AD-15).
- Aucune valeur de référence du jeu dans le code Rust ou React : tout passe par `reference/` et `GET /api/reference` (AD-6).
- Bundle : dépendances d'exécution React, react-dom, react-router, Tailwind seulement ; JS initial ≤ 150 ko gzip, CSS ≤ 20 ko gzip, contrôlés en CI (AD-19).
- Aucun workflow de pull request ni de fork n'atteint le runner auto-hébergé (AD-10).
- Versions de la table *Stack* de la spine.

## Non-goals

- Aucun écran métier : création, candidature, organisation, composition (epics 2 à 4).
- Pas d'administration, d'interrupteur `creation_enabled` ni de purge (epic 5).
- Pas de preuve de travail : la spine la diffère jusqu'au signal du forge ; ni feature Cargo ni solveur client.
- Pas de staging, pas d'E2E navigateur, pas de PWA, pas de mesure de performance en CI.
- Pas de réglage du runner, du VPS ni du DNS par l'agent : ce sont des gestes de Vincent, guidés par une procédure écrite.

## Success signal

- Après le dernier push de l'epic, l'URL publique (via le VPS) affiche l'accueil vide en français et en anglais sur un téléphone, `/api/health` est vert, Vincent change un libellé de classe dans `reference/`, pousse, et le voit dans `/api/reference` sans rebuild ; une relecture du compose déployé et des en-têtes ne trouve aucun point interdit de l'intent §4.

## Assumptions

- L'interrupteur `creation_enabled` et les routes `/api/admin/*` arrivent en epic 5 ; l'epic 1 ne livre que les seuils `MAX_EVENTS_PER_HOUR` et `MAX_CANDIDATURES_PER_HOUR`.
- La preuve de travail n'est pas construite en v1 (spine *Deferred*).
- `LinkSecret`, les classes de limiteur et le jeton de formulaire sont prouvés sur des routes de test ; leurs premières routes réelles arrivent en epic 2.
- Le mécanisme i18n (t(), locales, test de clés) est livré ici parce que la CI d'AD-5 l'exige ; les textes des écrans métier arrivent avec eux.
- Contenu initial de `reference/` : voir `reference-schema.md` (9 paires d'accès anticipé, 8 classes, 4 rôles, Ludra seule) ; libellés `fr` et tags `[SRV]` à vérifier dans le client.
- Bornes par classe par défaut neutres (min 0, max = taille du groupe).
- Seuls les secrets et `PUBLIC_BASE_URL` sont obligatoires ; les autres variables ont une valeur par défaut (voir `configuration.md`).

## Open Questions

- Quel est le format exact du tag `[SRV]` affiché par le client global (nom complet, abréviation) ? Il conditionne `Pseudo[SRV]`, geste de contact principal.
- Quelles versions de Docker Engine et Compose tournent sur le NAS (syntaxe `${VAR:?}`, compose sans clé `version`) ? À vérifier avant la story de déploiement.
- Conservation légale des données d'identification (forge, point faible 4) : toujours ouverte ; AD-7 et AD-15 s'appliquent tels quels.
