---
status: blocked
---

# Verrouiller et déverrouiller

## Auto Run Result

Status: blocked
Blocking condition: intent gap

La story crée la migration `0003` (`groups`, `assignments`), immuable une fois poussée. Deux décisions restent ouvertes et changent le schéma ou le comportement observable. Elles reviennent à Vincent.

1. **Re-verrouillage avec moins de groupes.** Trois énoncés d'AD-4 se contredisent :
   - K groupes égale la longueur de la liste retenue ;
   - déverrouiller ne supprime rien ;
   - seule la suppression d'un créneau fait perdre un groupe.

   Exemple : un raid est verrouillé avec 2 groupes, déverrouillé, puis reverrouillé avec 1 seul. Que devient le deuxième groupe ?
   - (a) **Il reste dormant.** Il garde son nom de salle et ses placements, il est masqué, et il est réactivé si on le reprend. Cela demande une colonne `retained` dans `groups`.
   - (b) **Il est supprimé**, avec ses placements.
2. **Longueur du nom de salle.** Le gabarit par défaut `{label}-{code} {cp}+` donne jusqu'à 21 caractères, par exemple `LUDRA-K7Q2 175000+`. La borne prévue, `room_label_max + 10` soit 18, est donc déjà dépassée. La longueur maximale et les caractères permis d'un nom de salle en jeu ne sont pas connus (question ouverte du SPEC). Quelle limite appliquer ? Et faut-il garder le CP dans le nom ?

Recherche faite et propositions : voir la conversation du 2026-11-07, investigation de la story 3-2.
