---
id: SPEC-epic-4-composition
companions:
  - solveur.md
  - ../spec-epic-3-organiser/placement-et-salle.md
  - ../../planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md
sources:
  - ../../forge/aion2-raid-sans-compte/forged-idea.md
---

> **Contrat canonique.** Ce SPEC et ses `companions:` forment le contrat complet. Les `AD-n` renvoient à
> `ARCHITECTURE-SPINE.md`, qui fait foi. Epics 1 à 3 supposés livrés.

# Epic 4 — Composition automatique

## Why

Opportunité, cas légion : avec quinze à quarante candidatures sur plusieurs créneaux et K groupes,
composer à la main prend du temps et déséquilibre les groupes. Un solveur déterministe, sans IA ni réseau,
propose le plus de groupes complets possible, équilibrés en CP, que l'organisateur corrige puis applique.
Construit en dernier par décision du forge : l'epic 3 a déjà prouvé le flux sans lui (AD-13 « optionnelle
au flux »).

## Capabilities

- **CAP-1**
  - **intent:** Le noyau calcule une proposition de placement à partir d'un instantané, toujours la même pour la même entrée, sans jamais violer une contrainte dure (AD-13, verrou Composition).
  - **success:** tests de propriétés sur `domain::compose` : même entrée → même `Proposal` ; aucun titulaire indisponible à son créneau ; aucune classe au-delà de son max dans un groupe ; au plus `parties × party_size` titulaires par groupe ; une candidature au plus une fois ; seules des candidatures `accepted` ; `Proposal` a exactement la forme d'entrée d'`apply_placement` et passe sa validation ; aucune I/O, aucun aléa (vérifié par l'absence de dépendance dans `crates/domain`).
- **CAP-2**
  - **intent:** La proposition sert d'abord le nombre de groupes complets, puis l'équilibre des CP (ou le plus fort d'abord), puis les créneaux préférés, et remplit le banc (verrou Composition, définitions dans `solveur.md`).
  - **success:** cas de test nommés : un vivier qui permet deux groupes complets en donne deux ; en mode équilibre, l'écart de CP entre groupes complets est minimal sur des vivier de référence résolus à la main ; en mode « le plus fort d'abord », G1 a la plus forte somme de CP ; à égalité, la préférence départage ; le banc suit les règles de `solveur.md`.
- **CAP-3**
  - **intent:** Le calcul ne bloque jamais le serveur et rend toujours une réponse dans son budget (AD-13).
  - **success:** test : sur un vivier de 200 candidatures et 14 créneaux, `compose` rend un résultat valide en ≤ `COMPOSE_TIME_BUDGET_MS` + 100 ms ; le calcul tourne en `spawn_blocking` ; une requête de santé pendant le calcul répond.
- **CAP-4**
  - **intent:** L'organisateur obtient une proposition en un geste, la voit en brouillon, la corrige et l'applique ou l'abandonne ; rien n'est écrit sans son geste (verrou Composition, EXPERIENCE *Écran de composition*).
  - **success:** onglet Composition : « Composer automatiquement » ouvre une feuille d'objectif (« Équilibrer les CP entre groupes » par défaut, « Le plus fort d'abord ») qui précise que rien n'est enregistré avant Appliquer ; « Calcul… » pendant l'attente ; la proposition remplit tous les groupes en brouillon, les slots changés portent le badge « Proposé », la barre devient « Appliquer » / « Abandonner » ; une correction à la main suit les règles de l'epic 3 ; aucun appel à `apply_placement` avant « Appliquer » (test) ; « Proposition : 2 groupes complets. Rien n'est enregistré tant que tu n'appliques pas. »

## Constraints

- Solveur dans `domain`, pur : aucune I/O, aucun aléa, aucun appel réseau ni modèle de langage ; départages par ordre de création (AD-13, verrou Composition).
- L'unité est la candidature : jamais de dédoublonnage par `Pseudo[SRV]` (AD-13).
- `min_cp` n'est jamais une contrainte du solveur (AD-4).
- Rien n'est écrit sans `apply_placement` explicite (AD-13).
- Écran de composition chargé à la demande, budget de poids AD-19 tenu.

## Non-goals

- Pas de rôle choisi par le joueur ni de bornes par rôle (spine *Deferred*).
- Pas d'optimalité prouvée : meilleur résultat dans le budget.
- Pas de proposition partielle par groupe : la proposition couvre tout l'événement.
- Pas de matchmaking d'inconnus au hasard : le solveur compose le vivier connu de l'événement.

## Success signal

- Sur un événement réel à deux groupes et plus de vingt candidatures acceptées, Vincent touche « Composer automatiquement », obtient en moins de deux secondes une proposition qu'il juge jouable, corrige au plus deux slots à la main et l'applique.

## Assumptions

- Max de classe dur pour tout groupe ; min dur seulement pour compter un groupe comme complet.
- Répartition des rôles au plus égal entre équipes, ordre des index et règles du banc : voir `solveur.md`.
- Déterminisme garanti par un nombre d'itérations borné ; le budget de temps d'AD-13 n'est qu'un filet, et une coupure par ce filet est signalée par `truncated: true` (tension d'AD-13 relevée, AD non modifié).
- Algorithme : construction gloutonne puis recherche locale par échanges jusqu'au budget ; seules les propriétés testées font foi.
- La proposition est demandée par un POST sans écriture, droit organisateur, classe `write`, en `locked` seulement.
