---
id: SPEC-epic-3-organiser
companions:
  - placement-et-salle.md
  - ../spec-epic-2-creer-candidater/commandes-et-droits.md
  - ../spec-epic-1-socle/reference-schema.md
  - ../../planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/mockups/key-grille.html
sources:
  - ../../forge/aion2-raid-sans-compte/forged-idea.md
  - ../../../docs/intent-dump.md
---

> **Contrat canonique.** Ce SPEC et ses `companions:` forment le contrat complet. Les `AD-n` renvoient à
> `ARCHITECTURE-SPINE.md`, qui fait foi. Epics 1 et 2 supposés livrés. La maquette illustre ; EXPERIENCE
> et DESIGN font foi en cas d'écart.

# Epic 3 — Organiser

## Why

Douleur : personne ne voit qui est disponible quand, et le jour J il faut encore retrouver chacun en jeu.
Cet epic ferme la boucle du booking : l'organisateur lit la grille, verrouille la date, place ses dix
titulaires et son banc à la main, obtient le nom de salle et la recette à reproduire en jeu, et colle un
message prêt dans Discord. À la fin, un vrai raid se monte de bout en bout sans solveur : c'est le signal
de succès du produit.

## Capabilities

- **CAP-1**
  - **intent:** L'organisateur voit d'un coup d'œil qui est disponible à quel créneau, sur téléphone, sans défilement horizontal (intent CAP-3, EXPERIENCE *Grille*, AD-19).
  - **success:** onglet Disponibilités : matrice si ≤ 4 créneaux et largeur utile ≥ 328 px, sinon vue par créneau, sélecteur grisé avec sa raison ; tri `accepted` d'abord puis nombre de créneaux cochés ; filtre Tous / Acceptés / En attente ; pied de colonne = disponibles parmi les `accepted` ; toucher un créneau affiche sa couverture par classe (« Clerc 1 / min 2 » en `danger` sous le minimum) ; vrai `<table>` avec `th scope` et cellules annoncées ; à 200 % de zoom, bascule en vue par créneau ; aucune largeur de 320 à 640 px ne défile horizontalement.
- **CAP-2**
  - **intent:** L'organisateur verrouille la date en deux touchers et obtient K groupes nommés, et peut revenir en préparation tant qu'un créneau est à venir (AD-4 *Verrouillage*, AD-18).
  - **success:** test HTTP : `lock_event` avec la liste des créneaux retenus (doublon permis) crée un groupe par entrée, fige `parties`, `party_size`, `room_label`, et donne un `room_name` unique parmi les groupes non purgés (collision → nouvel essai) ; `unlock_event` refusé si aucun créneau n'est à venir ; déverrouiller conserve groupes, placements et noms ; re-verrouiller ne nomme que les groupes nouveaux ; la feuille Verrouiller borne le stepper aux disponibles divisés par la taille de groupe ; en `locked`, la grille est en lecture seule et les onglets Composition et Salle apparaissent.
- **CAP-3**
  - **intent:** L'organisateur place à la main titulaires et banc de chaque groupe, y compris hors disponibilité ou au-delà d'une borne de classe, avec un avertissement (règle UX décidée, AD-4 *Placement*).
  - **success:** onglet Composition chargé à la demande (chunk séparé, budget AD-19 tenu) : toucher une cible ouvre la réserve ordonnée d'EXPERIENCE ; « Retirer », « Au banc », « Échanger » sur un slot occupé ; toute modification ouvre un brouillon local avec « Enregistrer » / « Abandonner », rien n'est envoyé avant ; « Enregistrer » envoie le placement complet de l'événement à `apply_placement`, qui le remplace en entier ; une violation structurelle (`placement-et-salle.md`) = 400, jamais une indisponibilité ni une borne ; le DTO organisateur porte les avertissements « pas dispo à ce créneau » et « 3 / max 2 », affichés en ligne ; une candidature n'est jamais dans deux slots (l'interface déplace) ; bandeau « N candidatures ont changé depuis ton brouillon » sans écraser le brouillon.
- **CAP-4**
  - **intent:** L'organisateur reproduit la salle en jeu sans rien retaper (verrou Rendez-vous en jeu, AD-18).
  - **success:** onglet Salle : une carte par groupe dans l'ordre des créneaux, nom de salle en `display` dans une puce à copier, recette en trois lignes (instance, nom, « Faction : All » si portée `all`), Équipe 1 puis Équipe 2 slot par slot avec numéro, rôle et `Pseudo[SRV]` à copier, puis le banc ; « Régénérer le nom » derrière « … » avec confirmation qui prévient que le nom posté deviendra faux.
- **CAP-5**
  - **intent:** L'organisateur poste la composition sur Discord en un geste (verrou Contact).
  - **success:** « Copier le message Discord » produit, depuis un gabarit des locales fr et en, l'instance, l'heure en `<t:UNIX:F>`, le nom de salle, les titulaires par équipe en `Pseudo[SRV]`, le banc et le lien public `/e/` ; test vitest : aucun secret de lien dans le message.
- **CAP-6**
  - **intent:** Chaque joueur sait, en rouvrant son lien, s'il joue, où et quand (verrou Contact).
  - **success:** en `locked`, `/p` affiche « Titulaire · Équipe 2, slot 4 · Soutien » ou « Banc · G1 · jeu. 21:00 » ou « Non placé », avec le créneau et le nom de salle à copier ; en `open`, ces informations disparaissent et « Le raid est revenu en préparation. » s'affiche ; la région live n'annonce que ces changements.

## Constraints

- Placement : une seule forme (table `assignments`, unique writer `apply_placement`, placement complet remplacé en entier) ; seules les candidatures `accepted` sont plaçables ; `withdraw` et `rejected` retirent le placement dans la même transaction (AD-4).
- Nom de salle composé côté serveur, indépendant de la langue, jamais côté client (AD-18).
- Interdits d'interaction : glisser-déposer, défilement horizontal, balayage pour agir, mises à jour qui déplacent la ligne sous le doigt (EXPERIENCE *Interaction Primitives*).
- Aucune information de fiabilité, d'absence passée ni de réputation (verrou Vitrine).
- Toute disposition (nombre d'équipes, taille, libellé de salle) vient du groupe figé, jamais d'une relecture de la référence (AD-4).

## Non-goals

- Pas de solveur ni de bouton « Composer automatiquement » (epic 4).
- Pas de demande de re-confirmation (epic 5).
- Pas de verrou optimiste : dernière écriture gagne (spine *Deferred*).
- Pas de notification ni d'appel à Discord : le message se copie (AD-17).

## Success signal

- Vrai raid Sanctuaire à 10 monté de bout en bout avec l'outil déployé : au moins deux serveurs, idéalement les deux factions, date sortie de la grille, dix titulaires placés à la main, message collé sur Discord, salle trouvée en jeu par son nom, slots conformes à la carte, raid joué ; personne n'a demandé « comment je me connecte ? ».

## Assumptions

- « Tri » se lit ici comme le tri et les filtres de la grille ; accepter et refuser sont livrés en epic 2.
- `ROOM_NAME_TEMPLATE` par défaut : `{label}-{code}`, suivi de ` {cp}+` si `min_cp` est défini (nouvelle variable de cet epic, voir `placement-et-salle.md`).
- Le brouillon de composition vit dans la mémoire de l'onglet ; un rechargement le perd.
- `ParticipantView` ne montre que le placement du participant et le nom de salle, pas la salle complète.

## Open Questions

- Quels sont la longueur maximale et le charset d'un nom de salle de Sanctuaire en jeu ? Ils bornent `ROOM_NAME_TEMPLATE` et la taille `display`.
