---
title: 'Sprint Change Proposal — coéquipiers visibles sur /p, badges de statut colorés'
date: '2026-11-07'
author: Vincent (via bmad-correct-course)
scope: minor
status: approved
---

# Sprint Change Proposal — 2026-11-07

## 1. Résumé du changement

**Déclencheur.** Le 7 novembre 2026, Vincent a essayé l'outil réel (fin de l'epic 2, déployé sur l'URL publique) depuis son téléphone. La candidature fonctionne. Deux demandes en sortent.

1. **Voir son équipe.** Un joueur accepté et placé ne voit aujourd'hui que sa propre place. C'est ce que prévoit l'epic 3 : SPEC *Assumptions*, « `ParticipantView` ne montre que le placement du participant et le nom de salle, pas la salle complète ». Vincent veut que chacun voie avec qui il joue.
   - **Titulaire :** il voit `Pseudo[SRV]` et la classe des autres titulaires de la même équipe, c'est-à-dire du même groupe et de la même Équipe.
   - **Banc :** décision de Vincent du 2026-11-07, « Oui, les joueurs sur le banc voient l'équipe de leur groupe ». Un joueur au banc voit les titulaires de son groupe.
2. **Badges colorés.** Les badges de statut demandés ont été livrés dans le commit `8e5ca60` :
   - `accepted` en vert ;
   - `pending` en bleu ;
   - `rejected` en rouge ;
   - `withdrawn` barré en gris ;
   - un badge affiché aussi pour `accepted` dans la ligne de candidature de `/o`.

   DESIGN.md ne décrit pas encore ces badges.

**Preuves.**
- Captures de Vincent sur téléphone (conversation du 2026-11-07).
- `_bmad-output/specs/spec-epic-3-organiser/SPEC.md` (*Assumptions*, CAP-6).
- Spine AD-4 *Données exposées*.
- `DESIGN.md` (*Ligne de candidature*, *Badge de statut*).
- Commit `8e5ca60`.

## 2. Analyse d'impact

### Checklist

| Item | Statut | Constat |
|---|---|---|
| 1.1 Déclencheur | [x] | Essai réel en production, demande produit de Vincent. |
| 1.2 Nature | [x] | Nouvelle exigence née de l'usage. Ce n'est ni un défaut ni une impasse technique. |
| 1.3 Preuves | [x] | Voir ci-dessus. |
| 2.1 Epic courant | [N/A] | L'epic 2 est terminé ; rien n'y change. |
| 2.2 Epics futurs | [!] | Epic 3 : CAP-6 et story 3-5. Epics 4 et 5 : aucun effet, la vue est déterminée par le placement, quel qu'en soit l'auteur (solveur ou manuel). |
| 2.3 Ordre et priorités | [x] | Aucun changement d'ordre. Tout reste dans 3-5. |
| 2.4 Nouvelles stories | [x] | Aucune. Le changement tient dans 3-5. |
| 2.5 Stories obsolètes | [N/A] | Aucune. |
| 3.1 PRD | [N/A] | Pas de PRD (chemin C allégé). Le contrat est porté par les SPEC d'epic. |
| 3.2 Architecture | [!] | AD-4 *Données exposées* : nouvelle exposition nominative restreinte, sous droit participant. |
| 3.3 UX | [!] | EXPERIENCE (*Ma candidature*, `/p` déverrouillé) et DESIGN (*Ligne de candidature*, *Badge de statut*). |
| 3.4 Autres artefacts | [x] | Le code de l'epic 2 n'est pas touché. Aucune migration : la donnée vient de `assignments`, qui arrive en 3-3. |
| 4.1 Ajustement direct | [x] | Viable, effort faible, risque faible. |
| 4.2 Retour arrière | [N/A] | Inutile. |
| 4.3 Revue du MVP | [N/A] | Le MVP n'est pas touché. |
| 4.4 Voie retenue | [x] | Ajustement direct, avant le build de l'epic 3. |
| 6.x Revue et passation | [x] | Voir §5. |

### Effets

- **Epic 3.** CAP-6 et la story 3-5 s'enrichissent : un champ en plus dans `ParticipantView`, et une section en plus sur `/p`. Les stories 3-1 à 3-4 ne changent pas.
- **Verrous de l'idée forgée.** Le verrou *Vitrine* (aucune fiabilité, réputation ni historique) n'est pas touché : on ne montre que `Pseudo[SRV]` et la classe. La règle « jamais de liste nominative » visait la page publique (`PublicEvent`). Elle reste entière : `/e` ne change pas.
- **Sécurité et vie privée.**
  - La liste n'est servie que sous droit participant (secret `/p`).
  - Elle n'est servie que si le participant est `accepted`, placé, et l'événement `locked`.
  - Elle ne contient que les joueurs `accepted` placés dans le même périmètre.
  - Un pending, un refusé ou un retiré ne voit rien et n'est jamais montré, puisqu'un refus ou un retrait supprime son placement.
- **Code déjà livré.** Seul DESIGN.md est à aligner sur le commit `8e5ca60`. Aucun autre code n'est touché.

## 3. Voie recommandée

**Ajustement direct**, à appliquer aux documents **avant** de lancer la boucle de l'epic 3 :
- **Effort :** faible, quelques phrases dans quatre documents.
- **Risque :** faible.
- **Planning :** aucun décalage, la story 3-5 grossit de quelques tâches.

Un argument décide : si l'epic 3 est construit avant cette mise à jour, la story 3-5 construira la règle actuelle, sans coéquipiers.

## 4. Propositions de modification

### 4.1 Architecture — `ARCHITECTURE-SPINE.md`, AD-4 *Données exposées*

ANCIEN :

```
  - *Données exposées* : `PublicEvent` (`/e/`, liste) porte les attributs de l'événement sans
    `discord_url` ni `note` longue, les créneaux et les comptes par classe et par rôle ; jamais la liste
    nominative (`[ASSUMPTION]`). `discord_url` n'est servi que sous droit organisateur ou sous droit
    participant `accepted`.
```

NOUVEAU :

```
  - *Données exposées* : `PublicEvent` (`/e/`, liste) porte les attributs de l'événement sans
    `discord_url` ni `note` longue, les créneaux et les comptes par classe et par rôle ; jamais la liste
    nominative (`[ASSUMPTION]`). `discord_url` n'est servi que sous droit organisateur ou sous droit
    participant `accepted`. Sous droit participant, et seulement pour une candidature `accepted` placée
    d'un événement `locked`, `ParticipantView` porte ses coéquipiers : `Pseudo[SRV]` et classe, rien
    d'autre, des autres titulaires de la même équipe (même groupe, même Équipe) pour un titulaire, des
    titulaires de son groupe pour un joueur au banc (décision de Vincent, 2026-11-07).
```

**Justification :** c'est une exposition nominative nouvelle, donc une décision d'architecture. Elle est bornée par le droit, le statut, le placement et l'état de l'événement.

### 4.2 SPEC de l'epic 3 — `spec-epic-3-organiser/SPEC.md`

**CAP-6, success.**

ANCIEN :

```
  - **success:** en `locked`, `/p` affiche « Titulaire · Équipe 2, slot 4 · Soutien » ou « Banc · G1 · jeu. 21:00 » ou « Non placé », avec le créneau et le nom de salle à copier ; en `open`, ces informations disparaissent et « Le raid est revenu en préparation. » s'affiche ; la région live n'annonce que ces changements.
```

NOUVEAU :

```
  - **success:** en `locked`, `/p` affiche « Titulaire · Équipe 2, slot 4 · Soutien » ou « Banc · G1 · jeu. 21:00 » ou « Non placé », avec le créneau et le nom de salle à copier ; un titulaire voit la liste « Ton équipe » (`Pseudo[SRV]` et classe des autres titulaires de son Équipe), un joueur au banc celle de son groupe (titulaires des deux Équipes, par Équipe) ; test HTTP : aucun coéquipier pour un participant non placé, non `accepted`, ou sur un événement `open`, et jamais d'autre champ que `pseudoSrv` et `class` ; en `open`, ces informations disparaissent et « Le raid est revenu en préparation. » s'affiche ; la région live n'annonce que ces changements.
```

***Assumptions*, quatrième puce.**

ANCIEN :

```
- `ParticipantView` ne montre que le placement du participant et le nom de salle, pas la salle complète.
```

NOUVEAU :

```
- `ParticipantView` montre le placement du participant, le nom de salle et ses coéquipiers (`Pseudo[SRV]` et classe : son Équipe pour un titulaire, les titulaires de son groupe pour un joueur au banc) ; jamais le banc aux titulaires, jamais la salle d'un autre groupe.
```

**Justification :** CAP-6 porte le critère vérifiable. L'hypothèse précédente contredisait la demande.

### 4.3 Stories de l'epic 3 — `spec-epic-3-organiser/stories.yaml`, story 3-5

ANCIEN :

```
    `ParticipantView` enrichi (placement, créneau, nom de salle) et état « revenu en préparation » sur
    `/p` ; fin d'epic : Vincent monte un vrai raid de bout en bout sans solveur (SPEC CAP-6, success signal).
```

NOUVEAU :

```
    `ParticipantView` enrichi (placement, créneau, nom de salle, coéquipiers en `Pseudo[SRV]` et classe)
    et état « revenu en préparation » sur `/p` ; fin d'epic : Vincent monte un vrai raid de bout en bout
    sans solveur (SPEC CAP-6, success signal).
```

`invoke_dev_with` gagne la phrase suivante, à la suite de l'existante :

```
    Coéquipiers : test HTTP qui vérifie qu'un participant non placé, non accepted, ou sur un événement
    open n'en reçoit aucun, et que chaque coéquipier n'a que `pseudoSrv` et `class`.
```

### 4.4 UX — `EXPERIENCE.md`

**Table d'architecture de l'information, ligne *Ma candidature*.**

ANCIEN : `statut, placement, salle, confirmation, modifier, se retirer (CAP-5, CONTACT)`

NOUVEAU : `statut, placement, salle, coéquipiers, confirmation, modifier, se retirer (CAP-5, CONTACT)`

**Ligne d'état *Événement déverrouillé*.**

ANCIEN : `Le placement et la salle disparaissent de la vue participant.`

NOUVEAU : `Le placement, la salle et les coéquipiers disparaissent de la vue participant.`

### 4.5 UX — `DESIGN.md`

***Ligne de candidature*.**

ANCIEN : `Badge de statut à droite si différent d'accepted.`

NOUVEAU : `Badge de statut à droite, pour tous les statuts.`

***Badge de statut*.**

ANCIEN : `` `pending` en bleu, `rejected` et `withdrawn` en `ink-disabled` barré, « Proposé » (placement du solveur non appliqué) en bordure or sur fond transparent. ``

NOUVEAU : `` bordure et texte de la couleur du statut sur un fond teinté à 10 % : `accepted` en `available` (vert), `pending` en `pending` (bleu), `rejected` en `danger` (rouge) ; `withdrawn` en `ink-disabled` barré ; le libellé accompagne toujours la couleur. « Proposé » (placement du solveur non appliqué) en bordure or sur fond transparent. ``

**Justification :** aligner DESIGN.md sur ce qui est livré (commit `8e5ca60`). C'est l'unique source du style des badges.

## 5. Passation

**Portée : mineure.** Ce sont des ajustements de documents avant le build de l'epic 3. Aucune réorganisation du backlog, aucun replan.

Chaque document garde son unique auteur (règle d'AGENTS.md, « un document = un writer »). Les modifications approuvées s'appliquent donc par le skill propriétaire, en mode mise à jour :

| Document | Skill propriétaire | Modifications |
|---|---|---|
| Spine (AD-4) | `bmad-architecture` | 4.1 |
| SPEC et `stories.yaml` de l'epic 3 | `bmad-spec` | 4.2 et 4.3 |
| `EXPERIENCE.md` et `DESIGN.md` | `bmad-ux` | 4.4 et 4.5 |

Ensuite, `/loop` construit l'epic 3.

**Critères de succès :**
- Le spine, le SPEC de l'epic 3, `stories.yaml`, EXPERIENCE et DESIGN contiennent le texte NOUVEAU ci-dessus.
- La story 3-5 livre la section « Ton équipe » sur `/p`, avec son test HTTP de confinement.
- La page publique `/e` reste sans aucun nom.
