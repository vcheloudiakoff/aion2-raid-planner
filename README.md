# aion2-raid-planner

Trouver du monde et fixer une date pour un raid (Sanctuaire) ou une expédition dans **Aion 2**, toutes
factions et tous serveurs confondus, **sans créer de compte**.

> Projet au stade **kickoff** (5 novembre 2026) : pas encore de code. L'intention est dans
> [`docs/intent-dump.md`](docs/intent-dump.md), la connaissance du jeu dans
> [`docs/domaine-aion2.md`](docs/domaine-aion2.md). Le développement suit la
> [méthode BMAD](https://github.com/bmad-code-org/BMAD-METHOD) v6.12, dont les skills sont versionnés ici.

## Le problème

Un Sanctuaire se joue à 10. Le jeu permet d'y aller inter-faction et inter-serveur, donc le vivier est
large, mais rien n'aide à se trouver ni à voir qui est disponible quand. Les raids se montent à coups de
messages épars sur Discord, et beaucoup ne se montent pas.

## Ce que l'outil fera

- Un organisateur publie un raid (instance, créneaux candidats, places par rôle) et obtient un lien.
- Un joueur rejoint depuis le lien ou la liste : pseudo, serveur, faction, classe, disponibilités.
- L'organisateur voit qui est libre quand, verrouille la date, complète la composition.
- Un joueur sans groupe trouve les raids ouverts où il manque son rôle.
- Tout cela sans compte, sans mot de passe, sans email.

## Ce qu'il ne fera pas

Pas de comptes, pas de chat (Discord fait ça), pas d'intégration avec le jeu, pas de gestion de légion ni
de loot, pas de matchmaking automatique, pas d'application native.

## Contraintes qui structurent tout

- **Hébergé sur un NAS personnel** : le service ne doit jamais devenir un tremplin vers le NAS. Conteneur
  non-root, jamais privilégié, aucun accès à l'hôte, secrets hors dépôt.
- **Zéro friction** : pas de login. L'identité tient sur des liens porteurs de droits et sur l'appareil.
- **Données minimales** : pseudo de jeu, serveur, faction, classe, disponibilités. Rien d'autre.
- **Solo, bénévole, 0 € récurrent**, mobile d'abord, français d'abord.

## Démarrer

Prérequis : Node ≥ 20.12, Python ≥ 3.10 avec `uv`, Claude Code. Les skills BMAD sont déjà dans
`.claude/skills/` et la configuration dans `_bmad/` (le fichier `_bmad/config.user.toml`, personnel,
se régénère avec `npx bmad-method install`).

Prochaine étape, dans un chat neuf ouvert à la racine du dépôt :

```
/bmad-help
Contexte : outil web sans compte pour organiser des raids Aion 2 (trouver 10 joueurs toutes factions et
tous serveurs, comparer les disponibilités, fixer une date), hébergé sur mon NAS perso. Je pars de zéro.
J'ai déjà : docs/intent-dump.md (intention, contraintes de sécurité, questions ouvertes Q1 à Q11) et
docs/domaine-aion2.md (connaissance du jeu, sourcée).
Contraintes : solo, 0 € récurrent, pas de login, surface d'attaque minimale sur le NAS, mobile d'abord,
français d'abord.
Objectif de cette session : décider du chemin (une session / epic / projet) et lancer le premier skill.
Réponds en français, recommande le prochain skill et propose de le lancer.
```

Chemin recommandé (voir `~/bmad-method/02-Tutoriel-BMAD.md` §5) : `/bmad-forge-idea` sur l'intent dump
pour attaquer la claim centrale (« sans compte, simple, sûr et sans spam »), puis `/bmad-product-brief`,
`/bmad-architecture` (spine : identité sans compte, isolation sur le NAS, stack), `/bmad-ux`, puis un
`/bmad-spec` par epic.

## Arborescence

```
AGENTS.md                 contexte d'implémentation pour les agents (CLAUDE.md l'importe)
docs/intent-dump.md       l'intention : pourquoi, pour qui, capacités, contraintes, questions ouvertes
docs/domaine-aion2.md     factions, serveurs, instances, classes, avec sources
_bmad/                    BMAD : config, scripts ; _bmad/custom/ porte les overrides committés
_bmad-output/             artefacts BMAD (brief, spine, specs, suivi) au fur et à mesure
.claude/skills/           les 29 skills BMAD pour Claude Code
```

## Licence

Pas encore choisie.
