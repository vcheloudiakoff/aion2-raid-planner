---
id: SPEC-epic-5-finitions
companions:
  - ../spec-epic-1-socle/configuration.md
  - ../spec-epic-2-creer-candidater/commandes-et-droits.md
  - ../spec-epic-3-organiser/placement-et-salle.md
  - ../../planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/EXPERIENCE.md
  - ../../planning-artifacts/ux-designs/ux-aion2-raid-planner-2026-11-05/DESIGN.md
sources:
  - ../../forge/aion2-raid-sans-compte/forged-idea.md
  - ../../../docs/intent-dump.md
---

> **Contrat canonique.** Ce SPEC et ses `companions:` forment le contrat complet. Les `AD-n` renvoient à
> `ARCHITECTURE-SPINE.md`, qui fait foi. Epics 1 à 4 supposés livrés.

# Epic 5 — Finitions

## Why

Trois manques restent après l'epic 4. Les absents du jour J sont la cause d'annulation numéro un, et le
forge les absorbe par la re-confirmation et le banc, sans aucune fiabilité affichée. Les joueurs sans
groupe ne trouvent un raid que si on leur donne le lien. Enfin, l'exploitant n'a aucun levier sur un abus,
et les données ne disparaissent pas d'elles-mêmes. Cet epic ferme la v1 : la boucle des absents, la
découverte par la liste publique (un pari, pas une promesse), l'administration minimale et la purge
automatique.

## Capabilities

- **CAP-1**
  - **intent:** L'organisateur demande aux joueurs choisis de confirmer avant une échéance, et chacun répond depuis son lien (verrou Vitrine, AD-4 *Confirmation*, AD-11).
  - **success:** test HTTP : `request_confirmation(candidatures, confirm_by)` écrase la ligne de chaque candidature ; `answer_confirmation` après `confirm_by` = 409 `confirmation.deadline_passed` ; l'état `unasked | asked | confirmed | declined | expired` sort de la seule `domain::confirmation_state(...)`, jamais stocké ; `expired` ne change aucun placement ; sur `/p`, carte en tête « Tu confirmes pour mar. 21:00 ? » Oui / Non avec échéance locale, puis « Trop tard pour répondre. Préviens l'organisateur sur Discord. » après l'échéance ; dans Salle, la feuille « Demander confirmation » coche les titulaires par défaut et propose « 2 h avant », « la veille » ou une heure libre.
- **CAP-2**
  - **intent:** Quand un titulaire décline ou laisse expirer, l'organisateur le remplace par un remplaçant du banc en quelques touchers (flux 4 d'EXPERIENCE).
  - **success:** Salle et Composition affichent l'état de confirmation de chaque titulaire et remplaçant (badge « expirée » en `danger`, « confirmé », « décliné ») ; déplacer un remplaçant du banc vers le slot libéré et enregistrer se fait en au plus quatre touchers ; le remplaçant lit ensuite « Titulaire · Équipe 2, slot 4 · Soutien » et le nom de salle sur `/p`.
- **CAP-3**
  - **intent:** Un joueur sans groupe trouve les raids ouverts qui lui correspondent sur l'accueil (intent CAP-4, verrou Public).
  - **success:** la liste ne montre que les événements `listed`, sans `admin_unlisted_at`, en `open`, avec au moins un créneau à venir, calculés à la lecture (AD-14) ; `PublicEventSummary` sans liste nominative ; filtres en puces par instance, rôle manquant, faction et jour, sans défilement horizontal ; tri par prochain créneau ; réponse de l'API en < 300 ms avec 500 événements en base et liste filtrée affichée en < 2 s en 4G ; liste vide = « Aucun raid ouvert pour l'instant. » + « Créer un raid ».
- **CAP-4**
  - **intent:** Vincent retire, supprime ou bloque sans compte ce qui abuse de l'outil (verrou Anti-abus, AD-3, AD-7).
  - **success:** `ADMIN_SECRET` absent = routes `/api/admin/*` en 404 ; présent, `/a/<secret>` liste les événements (`AdminEvent` sans pseudo ni candidature nominative), retire de la liste (`admin_unlisted_at`), supprime en cascade après confirmation destructive, coupe ou rétablit la création ; création coupée = 503 et message « La création de raids est en pause. Les raids existants marchent toujours. » ; l'état de l'interrupteur survit à un redémarrage ; le secret admin est comparé en temps constant et jamais journalisé.
- **CAP-5**
  - **intent:** Les raids passés et annulés disparaissent seuls, base et sauvegardes comprises (intent CAP-7, AD-14).
  - **success:** test avec `Clock` injectée : un événement est purgé en cascade à la première exécution horaire après `PURGE_AFTER_DAYS` jours suivant la fin de son dernier créneau ou son annulation ; aucun événement ne vit plus de `PURGE_AFTER_DAYS` + 2 jours en base et sauvegardes ; un lien purgé affiche le message unique de lien invalide et retire l'entrée locale ; la tâche ne touche à aucun statut ni confirmation.

## Constraints

- Aucune fiabilité, note, historique ni compteur d'absences, jamais, y compris dans l'admin (verrou Vitrine).
- `expired` est une information, jamais une action automatique sur le placement (AD-4).
- L'administration ne possède que `admin_unlisted_at`, la suppression et l'interrupteur de création (AD-4 *Propriétaires*).
- Aucune notification : le joueur rouvre son lien, l'organisateur prévient sur Discord (verrou Contact, AD-17).
- La liste publique ne montre jamais de pseudo (AD-4 *Données exposées*).

## Non-goals

- Pas de preuve de travail, même si l'admin l'appellerait : elle attend le signal du forge (nettoyage routinier).
- Pas de bouton signaler, pas de défi humain tiers (verrou Anti-abus).
- Pas d'export/import d'événement, de disponibilités récurrentes ni de liste de joueurs seuls (hors v1).
- Pas de journal d'IP : la question juridique reste ouverte, AD-7 et AD-15 s'appliquent tels quels.

## Success signal

- Sur un raid réel verrouillé, un titulaire ne confirme pas à l'échéance, l'organisateur voit « expirée », place un remplaçant du banc, et la salle est pleine à l'heure sans qu'aucun message n'ait été envoyé par l'outil ; une semaine plus tard, l'événement a disparu de la base et de `/e/`.

## Assumptions

- `creation_enabled` est persisté en base pour survivre à un redémarrage.
- Filtres de la liste : instance, rôle manquant, faction, jour ; région implicite tant qu'une seule est activée ; tri par prochain créneau.
- `AdminEvent` ne porte aucune donnée nominative de joueur.
- Si la story 1-6 n'a pas livré la variante admin de `LinkSecret`, la story 5-3 l'ajoute.

## Open Questions

- Conservation légale des données d'identification d'un site public à contenu visiteur (forge, point faible 4) : à trancher avant d'ouvrir l'outil au-delà de la légion.
