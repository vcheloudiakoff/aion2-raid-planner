---
name: aion2-raid-planner
description: Outil de booking de raids, sobre et dense, sombre par défaut, lisible d'une main sur un téléphone de 360 px à côté du jeu et de Discord.
status: final
created: '2026-11-05'
updated: '2026-11-07'
sources:
  - _bmad-output/forge/aion2-raid-sans-compte/forged-idea.md
  - _bmad-output/planning-artifacts/architecture/architecture-aion2-raid-planner-2026-11-05/ARCHITECTURE-SPINE.md
  - docs/intent-dump.md
  - docs/domaine-aion2.md
colors:
  surface-base-dark: '#121418'
  surface-raised-dark: '#1B1E24'
  surface-sunken-dark: '#0D0F12'
  ink-primary-dark: '#E9EBEF'
  ink-secondary-dark: '#A3A9B4'
  ink-disabled-dark: '#5D636D'
  border-dark: '#2C313A'
  accent-dark: '#E0B04A'
  on-accent-dark: '#121418'
  available-dark: '#4CC38A'
  pending-dark: '#7AAEEF'
  danger-dark: '#EF7A70'
  faction-elyos-dark: '#8CC2F0'
  faction-asmodian-dark: '#C79BE6'
  surface-base: '#F6F6F3'
  surface-raised: '#FFFFFF'
  surface-sunken: '#ECECE8'
  ink-primary: '#17191D'
  ink-secondary: '#575D67'
  ink-disabled: '#A5AAB2'
  border: '#D8DADE'
  accent: '#8A5E0A'
  on-accent: '#FFFFFF'
  available: '#1C7449'
  pending: '#2A5C9C'
  danger: '#B3261E'
  faction-elyos: '#2A6AA6'
  faction-asmodian: '#7438A0'
typography:
  display:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
    fontSize: 24px
    fontWeight: 700
    lineHeight: 1.25
  title:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
    fontSize: 20px
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
    fontSize: 14px
    fontWeight: 600
    lineHeight: 1.35
  meta:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: 'ui-monospace, "SF Mono", "Cascadia Mono", Menlo, Consolas, monospace'
    fontSize: 15px
    fontWeight: 500
    lineHeight: 1.4
rounded:
  sm: 4px
  md: 8px
  lg: 12px
  full: 9999px
spacing:
  '1': 4px
  '2': 8px
  '3': 12px
  '4': 16px
  '5': 24px
  '6': 32px
  gutter-mobile: 16px
  tap-min: 44px
  content-max: 640px
components:
  button-primary:
    background: '{colors.accent-dark}'
    text: '{colors.on-accent-dark}'
    typography: '{typography.label}'
    rounded: '{rounded.md}'
    minHeight: '{spacing.tap-min}'
  button-secondary:
    background: transparent
    border: '{colors.border-dark}'
    text: '{colors.ink-primary-dark}'
    rounded: '{rounded.md}'
    minHeight: '{spacing.tap-min}'
  copy-chip:
    background: '{colors.surface-sunken-dark}'
    text: '{colors.ink-primary-dark}'
    typography: '{typography.mono}'
    rounded: '{rounded.sm}'
    minHeight: '{spacing.tap-min}'
  slot-chip:
    background: '{colors.surface-raised-dark}'
    selectedBorder: '{colors.accent-dark}'
    typography: '{typography.label}'
    rounded: '{rounded.full}'
    minHeight: '{spacing.tap-min}'
  grid-cell:
    size: '{spacing.tap-min}'
    available: '{colors.available-dark}'
    unavailable: '{colors.surface-sunken-dark}'
    rounded: '{rounded.sm}'
  room-slot:
    background: '{colors.surface-raised-dark}'
    border: '{colors.border-dark}'
    rounded: '{rounded.md}'
    minHeight: 52px
  status-badge:
    typography: '{typography.meta}'
    rounded: '{rounded.full}'
  bottom-sheet:
    background: '{colors.surface-raised-dark}'
    rounded: '{rounded.lg}'
---

## Brand & Style

Un outil de raid, pas un site de fan. On l'ouvre entre deux pulls, d'une main, Discord dans l'autre
onglet, souvent le soir. Il doit se lire en deux secondes et se taire le reste du temps. La posture est
celle d'un tableau de service : dense, net, factuel. Ni lore, ni illustration, ni effet.

Le jeu n'apparaît que par ses données : noms de serveurs, de classes, d'instances et de factions, tous
lus dans la référence. L'outil n'utilise aucun logo, aucune police, aucune image ni aucun pictogramme du
jeu. C'est un outil communautaire sur un dépôt public : il ne doit ni imiter la marque de l'éditeur ni
paraître officiel.

Le sombre est le thème par défaut, parce que c'est le contexte d'usage. Le clair est un réglage, et le
thème suit `prefers-color-scheme` tant que le joueur n'a rien choisi.

## Colors

La palette est neutre et reçoit **un seul accent**, l'or (`{colors.accent-dark}` / `{colors.accent}`).
L'or rappelle vaguement l'univers sans rien emprunter. Il est réservé à l'action principale d'un écran et
à la sélection en cours : le créneau choisi, la cible de placement.

- **Surfaces** : trois niveaux. `surface-sunken` sert de fond aux cellules vides et aux puces à copier,
  `surface-base` au canevas, `surface-raised` aux cartes, aux feuilles et aux slots de salle. La
  hiérarchie se fait par le ton, pas par l'ombre.
- **Encre** : `ink-primary` pour le texte, `ink-secondary` pour les métadonnées (serveur, CP, heure
  locale), `ink-disabled` pour un créneau passé ou une candidature retirée.
- **Disponible** (`{colors.available-dark}`) : une cellule cochée de la grille, une confirmation
  « oui ». C'est la seule couleur « positive » de l'outil.
- **En attente** (`{colors.pending-dark}`) : une candidature `pending`, une confirmation `asked`.
- **Danger** (`{colors.danger-dark}`) : refuser, annuler l'événement, supprimer un créneau qui a des
  groupes, échéance dépassée. Le danger apparaît en texte et en bordure, jamais en grand aplat.
- **Factions** (`faction-elyos`, `faction-asmodian`) : seulement un point de 8 px et le nom de la faction
  à côté du serveur. Ce n'est jamais un fond ni un code de statut.

Les rôles n'ont pas de couleur propre : une couleur par rôle s'ajouterait à celles des factions et des
statuts, ce qui ferait trois codes concurrents sur 360 px. Le rôle se lit par son libellé court
(`Tank`, `Soin`, `DPS`, `Soutien`, d'après la référence).

Contraste visé : 4,5:1 pour le texte et 3:1 pour les bordures d'état et les cellules, dans les deux
thèmes. Les valeurs ci-dessus sont choisies pour cela et se vérifient à l'implémentation.

## Typography

Polices système seulement : la CSP stricte interdit les polices tierces (AD-9) et chaque kilo-octet
compte (AD-19). Le corps de texte est à 16 px, ce qui évite aussi le zoom automatique d'iOS sur les champs
de saisie.

- `display` : le nom de la salle sur la recette en jeu. C'est le seul très gros texte de l'outil.
- `title` : le titre d'écran (instance + date).
- `label` : boutons, puces, en-têtes de colonne de la grille.
- `meta` : serveur, CP, heure locale, compteurs.
- `mono` : tout ce qui se recopie à l'identique dans le jeu, c'est-à-dire `Pseudo[SRV]` et le nom de
  salle.

Les chiffres (CP, compteurs, heures) utilisent `font-variant-numeric: tabular-nums` pour rester alignés
en colonne. Aucun libellé en capitales.

## Layout & Spacing

Le pas est de 4 px (`{spacing.1}` à `{spacing.6}`). La gouttière est de 16 px sur mobile
(`{spacing.gutter-mobile}`). Au-delà de 640 px, le contenu se centre dans une colonne de
`{spacing.content-max}` : la v1 n'a pas de mise en page bureau distincte, le PC affiche la vue mobile
élargie.

La largeur de référence est 360 px, et **aucun écran ne défile horizontalement** (AD-19). Il reste 328 px
utiles. La grille joueurs × créneaux s'y résout ainsi (le détail du comportement est dans EXPERIENCE.md) :

- **Matrice** jusqu'à 4 créneaux : une colonne joueur de 136 px, puis 4 colonnes de 44 px séparées de
  4 px.
- **Vue par créneau** au-delà.

Chaque cible tactile fait au moins `{spacing.tap-min}`.

## Elevation & Depth

Pas d'ombre portée, sauf sous la feuille du bas (`bottom-sheet`) et le toast de copie, qui flottent
au-dessus du contenu. Tout le reste se distingue par le ton de surface et une bordure de 1 px
`{colors.border-dark}`.

## Shapes

- `{rounded.sm}` (4 px) : cellules de grille, puces à copier.
- `{rounded.md}` (8 px) : boutons, cartes, slots de salle.
- `{rounded.lg}` (12 px) : bord haut de la feuille du bas.
- `{rounded.full}` : puces de créneau et badges de statut, les seuls éléments en pilule. Ce sont des
  filtres et des étiquettes, pas des actions.

## Components

- **Bouton principal** (`button-primary`) : fond or, texte sombre. Un seul par écran. En bas de l'écran
  sur les formulaires mobiles, collé au-dessus du clavier si possible.
- **Bouton secondaire** : bordure seule. Variante danger : bordure et texte `danger`.
- **Puce à copier** (`copy-chip`) : texte en `mono` sur `surface-sunken`, avec une icône « copier » à
  droite. C'est le composant le plus important de l'outil (geste de contact principal du forge). Toute
  la puce est la cible du toucher.
- **Puce de créneau** (`slot-chip`) : jour court + heure locale + compte de disponibles (« Mar 21:00 ·
  12 »). Sélectionnée : bordure or de 2 px. Créneau passé : `ink-disabled`.
- **Cellule de grille** (`grid-cell`) : 44 × 44 px. Disponible : fond `available` et coche ✓. Non
  disponible : fond `sunken` et tiret. Créneau préféré : petite étoile en coin.
- **Ligne de candidature** : `Pseudo` en `label`, `[SRV]` et classe en `meta`, CP aligné à droite en
  chiffres tabulaires. Point de faction devant le serveur. Badge de statut à droite, pour tous les
  statuts. Doublon `Pseudo[SRV]` : badge « doublon » en `danger`.
- **Slot de salle** (`room-slot`) : numéro du slot, rôle attendu en `meta`, puis le titulaire
  (`Pseudo[SRV]` en `mono`) ou « libre » en `ink-disabled`. Cible de placement active : bordure or
  pointillée.
- **Badge de statut** : bordure et texte de la couleur du statut sur un fond teinté à 10 % : `accepted` en
  `available` (vert), `pending` en `pending` (bleu), `rejected` en `danger` (rouge) ; `withdrawn` en
  `ink-disabled` barré ; le libellé accompagne toujours la couleur. « Proposé »
  (placement du solveur non appliqué) en bordure or sur fond transparent.
- **Feuille du bas** : la réserve des non-placés en composition, et les choix de langue et de thème.
  Poignée visible, fermeture par glissement vers le bas ou par le bouton Fermer.
- **Toast** : « Copié » pendant 1,5 s, en bas, au-dessus du bouton principal.

## Do's and Don'ts

| Faire | Ne pas faire |
|---|---|
| Un seul accent or, sur l'action principale et la sélection | Colorer les rôles, les classes ou les serveurs |
| Doubler chaque couleur d'état d'un symbole ou d'un mot | Disponibilité signalée par la couleur seule |
| `mono` pour tout ce qui se recopie dans le jeu | Recomposer `Pseudo[SRV]` avec une autre ponctuation |
| Polices système, aucune image de décor | Logo, police ou art du jeu ; arrière-plans illustrés |
| Ton de surface et bordure 1 px pour la hiérarchie | Ombres en cascade, dégradés, verre dépoli |
| Tenir dans 328 px utiles, avec des cibles de 44 px | Défilement horizontal, cibles de moins de 44 px |
