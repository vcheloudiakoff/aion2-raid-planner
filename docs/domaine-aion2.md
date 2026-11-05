# Aion 2 — connaissance du domaine utile au produit

> Rassemblé le 05/11/2026 par recherche web, pour nourrir le brief, l'UX et les données de référence.
> Tout ce qui suit est **à vérifier dans le client du jeu** avant d'en faire une donnée de référence :
> les sources tierces se contredisent sur les listes de serveurs, et NCSoft ajuste souvent.

## Le jeu et sa sortie

- MMORPG de NCSoft, suite d'Aion. Sortie en Corée et à Taïwan le 19 novembre 2025.
- Sortie **globale** (Europe, Amériques, Asie) le **5 octobre 2026** (13:00 UTC, 15:00 heure de Paris),
  après un accès anticipé fin septembre. Jeu en français sur les serveurs globaux.

## Factions et serveurs

- Deux factions : **Élyséens** (Elyos) et **Asmodiens** (Asmodians).
- **Un serveur n'accueille qu'une seule faction.** Chaque serveur Élyséen est apparié à un serveur Asmodien
  pour le JcJ (l'Abysse). Les appariements sont présentés comme temporaires, ajustables pour l'équilibre.
- Choix à la création : région → faction → serveur.
- Cinq régions de serveurs : Europe, Amérique du Nord Ouest, Amérique du Nord Est, Amérique latine, Asie.
- Europe : entre 22 et 23 paires selon la source et la date (9 paires d'accès anticipé, 13 ou 14 paires
  de lancement). Les paires d'accès anticipé, cohérentes entre sources :

| Élyséens | Asmodiens |
|---|---|
| Siel | Israphel |
| Nezekan | Zikel |
| Vaizel | Triniel |
| Kaisinel | Lumiel |
| Yustiel | Marchutan |
| Ariel | Azphel |
| Fregion | Ereshkigal |
| Meslamtaeda | Beritra |
| Hithanya | Nemon |

  Noms de serveurs de lancement cités (appariements contradictoires selon les sources, à ne pas recopier
  sans vérification) : Élyséens Nania, Tahavatha, Luteros, Phernos, Daminu, Kasaka, Bakarma, Tsenka,
  Kochi, Ishtar, Tiamat, Gauss, Lamuatan, Nathara ; Asmodiens Hadala, Ludra, Ulgorn, Munin, Odar,
  Zemurru, Kromede, Quai, Baba, Fafnir, Indnath, Agnita, Atiel, Tassin.
- Conséquence produit : la liste des serveurs doit être une **donnée éditable**, par région et par
  faction, jamais du code.

## Instances : expéditions et sanctuaires

| Type | Taille de groupe (depuis le chapitre 1) | Nature |
|---|---|---|
| **Expédition** | 5 joueurs (4 avant) ; certaines jouables en solo, d'autres à 2 minimum | donjon de groupe |
| Transcendance | 5 joueurs | donjon de groupe, coût en énergie d'Odyle |
| **Sanctuaire** | **10 joueurs**, soit deux groupes de 5 (8 avant) | raid, « contenu de légion ou de groupe prédéfini » en Corée |

- Le contenu instancié est **inter-serveur**.
- Sur la version globale, Élyséens et Asmodiens **peuvent faire équipe dans les donjons** ; une option
  permet de ne matcher qu'avec sa propre faction. L'inter-faction ne s'applique qu'au contenu JcE qui le
  prend en charge : vérifier les options de groupe de l'activité.
- À confirmer en jeu (Q6 de l'intent dump) : un groupe prédéfini de 10 venus de serveurs différents peut-il
  entrer ensemble, ou seul le matchmaking est inter-serveur ?
- Sanctuaire global : **Raid de Sanctuaire : Ludra**, retiré au lancement du 5 octobre, revenu le
  **4 novembre 2026** avec de nouvelles mécaniques. Sanctuaires cités côté Corée : Forge abyssale, Centre
  de décontamination corrodé, Calice de Muspel, Champ de neige du chagrin (traductions libres, noms FR à
  relever dans le client).
- Limites hebdomadaires (version globale, chapitre 1) : expéditions sans ticket d'entrée, une récompense
  récupérée toutes les 8 h cumulable jusqu'à 21 ; sanctuaire sans limite de tentatives mais deux
  récompenses par semaine. Utile pour l'UX : un joueur peut vouloir plusieurs raids par semaine.

## Classes

Huit classes communes aux deux factions. Noms français d'après Aion premier du nom, **à vérifier dans le
client d'Aion 2** :

| Anglais | Français (à vérifier) | Rôle habituel |
|---|---|---|
| Gladiator | Gladiateur | DPS mêlée |
| Templar | Templier | Tank |
| Assassin | Assassin | DPS mêlée |
| Ranger | Rôdeur | DPS distance |
| Sorcerer | Sorcier | DPS magique |
| Spiritmaster (ou Elementalist selon la source) | Spiritualiste | DPS magique, invocateur |
| Cleric | Clerc | Soigneur |
| Chanter | Aède | Soutien, soins secondaires |

Une neuvième classe est annoncée côté Corée (nommée Kwon-seong ou Brawler selon la source) ; sa présence
sur la version globale n'est pas confirmée. Conséquence produit : classes et rôles sont des données
éditables, et une inscription doit pouvoir indiquer le **rôle** (tank, soin, DPS) autant que la classe.

## Sources (consultées le 05/11/2026)

- Serveurs et appariements : https://aion2guides.com/en/updates/global-launch-servers ·
  https://space4games.com/en/games-en/aion-2-server-guide/ · https://senseos.gg/guides/aion-2/server-list/ ·
  https://www.playnews.gg/guides/serveurs-adverses-aion-2-quel-serveur-affronte-le-votre-abysse ·
  https://mein-mmo.de/en/aion-2-servers-for-europe-all-info-on-launch-matchmaking-transfer-and-market,1590045/
- Sortie globale : https://www.dexerto.fr/jeux-video/aion-2-informations-1660374/ ·
  https://www.u4n.com/news/aion-2-global-release-date-server-list.html
- Instances, tailles, limites : https://metaroad.gg/aion2/dungeons/aion-2-dungeon-system-expedition-transcendence-sanctuary ·
  https://aion2builds.com/guides/dungeons/ · https://aion2.wiki.fextralife.com/Dungeons ·
  https://www.playnews.gg/actualites/aion-2-livestream-mondial-donjons-5-raids-10-cinq-regions-serveurs
- Inter-faction et inter-serveur : https://gamerblurb.com/guides/aion-2-how-to-play-with-friends ·
  https://www.iggm.com/news/aion-2-global-servers-dungeon-gameplay-changes-expeditions-gear-progression-stigma-slots
- Ludra : https://elitegrindlab.com/aion-2/dungeons/ · https://metaroad.gg/aion2/getting-started/aion-2-global-build-season-1-chapter-1
- Classes : https://www.gamewatcher.com/guides/aion-2-classes-and-tier-list · https://www.exitlag.com/blog/aion-2-characters/ ·
  https://www.gameblog.fr/jeu-video/ed/astuces/aion-2-tier-list-meilleures-classes-724605
