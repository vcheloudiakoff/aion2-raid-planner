# Solveur — définitions et règles

## Entrée et sortie (AD-13)

- Entrée : groupes verrouillés (créneau, `parties`, `party_size` figés) dans l'ordre des créneaux puis de
  création ; candidatures `accepted` (ordre de création) avec disponibilités, préférences, classe, rôle,
  CP ; `class_bounds` ; objectif `balance` | `strongest_first`.
- Sortie `Proposal` : par groupe, titulaires `(party, index)` et banc ordonné ; candidatures absentes =
  `unplaced`. Même forme que l'entrée d'`apply_placement`.

## Contraintes dures

1. Un titulaire ou un remplaçant est disponible au créneau de son groupe.
2. Par groupe, le nombre de titulaires de chaque classe ≤ son `max`.
3. Au plus `parties × party_size` titulaires par groupe.
4. Une candidature apparaît au plus une fois dans toute la proposition.

## Objectif, ordre lexicographique

1. **Groupes complets** : maximiser le nombre de groupes ayant tous leurs slots pourvus et chaque classe
   ≥ son `min` `[ASSUMPTION]`.
2. **Selon l'objectif** :
   - `balance` : minimiser l'écart entre la plus haute et la plus basse somme de CP des titulaires parmi
     les groupes complets ;
   - `strongest_first` : maximiser la somme de CP de G1, puis de G2, et ainsi de suite.
3. **Préférence** : maximiser le nombre de titulaires placés sur un créneau qu'ils ont marqué préféré
   (plusieurs préférences par candidature, règle UX).
4. **Remplissage** : maximiser le nombre de titulaires dans les groupes incomplets.
5. Départage final : ordre de création des candidatures, puis des groupes.

## Disposition dans le groupe `[ASSUMPTION]`

- Rôles répartis au plus égal entre les équipes (un soin par équipe quand il y en a deux).
- Dans une équipe, index ordonnés tank, healer, support, dps, puis CP décroissant.

## Banc `[ASSUMPTION]`

- Candidatures `accepted` non titulaires, disponibles au créneau du groupe.
- Au plus `party_size` remplaçants par groupe ; une candidature sur un seul banc, d'abord celui d'un groupe
  à créneau préféré, sinon le premier groupe possible.
- Ordre du banc : CP décroissant, puis ordre de création.

## Budget

Tension relevée dans AD-13 : une coupure au temps rend le résultat dépendant de la vitesse de la machine,
donc non déterministe. Résolution `[ASSUMPTION]` :

- la recherche est bornée par un **nombre d'itérations** déterministe (`COMPOSE_MAX_ITERATIONS`, constante
  du `domain` calibrée pour tenir dans 1000 ms sur le NAS pour 200 candidatures et 14 créneaux) ; c'est
  lui qui garantit « même entrée, même sortie » ;
- `COMPOSE_TIME_BUDGET_MS` (défaut 1000, voir `configuration.md` de l'epic 1) reste un filet : `app` passe
  à `domain` une échéance via un prédicat injecté, jamais l'horloge système ; si le filet coupe avant la
  fin des itérations, le meilleur résultat courant est rendu et le DTO `Proposal` porte `truncated: true`.
