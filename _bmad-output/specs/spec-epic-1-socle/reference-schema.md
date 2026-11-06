# Référence du jeu — schéma TOML et contenu initial

Cinq fichiers dans `reference/` (copiés dans `/data/reference/` par le runner), les seuls lus (AD-6).
Règles communes : clé = slug opaque en anglais, jamais renommé ; retrait par `enabled = false`, jamais par
suppression ; `label.en` obligatoire, `label.fr` optionnel avec repli sur `en`. Le validateur
(`validate-reference`, démarrage, rechargement, CI) rejette : id dupliqué, référence croisée vers un id
absent, `label.en` manquant, valeur hors bornes ci-dessous.

| Fichier | Champs par entrée | Contrôles |
| --- | --- | --- |
| `regions.toml` | `id`, `label`, `enabled` | — |
| `servers.toml` | `id`, `region`, `faction` (`elyos` \| `asmodian`), `tag`, `label`, `enabled` | `region` existe ; `tag` non vide, sans `[` ni `]`, ≤ 16 caractères `[ASSUMPTION]` ; `tag` unique par région |
| `instances.toml` | `id`, `kind` (`sanctuary` \| `expedition`), `parties`, `party_size`, `duration_min`, `cross_faction`, `room_label`, `label`, `enabled` | `parties` ≥ 1, `party_size` ≥ 1, `duration_min` > 0 ; `room_label` dans le charset et la longueur déclarés en tête du fichier (`room_label_charset`, `room_label_max`) |
| `classes.toml` | `id`, `role`, `default_min`, `default_max`, `label`, `enabled` | `role` existe ; 0 ≤ `default_min` ≤ `default_max` |
| `roles.toml` | `id`, `label` (dont libellé court : `Tank`, `Soin`, `DPS`, `Soutien`), `enabled` | — |

Les factions (`elyos`, `asmodian`) sont les deux seules valeurs du domaine qui ne sont pas des données :
la règle « un serveur = une faction » et `faction_scope` en dépendent. Leurs libellés vivent dans un
tableau `[factions]` en tête de `servers.toml` `[ASSUMPTION]`, pour qu'aucun libellé ne soit dans le code
et que la liste des cinq fichiers d'AD-6 reste fermée.

## Contenu initial `[ASSUMPTION]`, à vérifier dans le client par Vincent

- **Régions** : Europe (activée) ; Amérique du Nord Ouest, Amérique du Nord Est, Amérique latine, Asie
  (`enabled = false` tant qu'aucun serveur n'y est saisi).
- **Serveurs Europe** : les 9 paires d'accès anticipé de `docs/domaine-aion2.md` (Siel/Israphel,
  Nezekan/Zikel, Vaizel/Triniel, Kaisinel/Lumiel, Yustiel/Marchutan, Ariel/Azphel, Fregion/Ereshkigal,
  Meslamtaeda/Beritra, Hithanya/Nemon). `tag` = nom du serveur en attendant le relevé. Les serveurs de
  lancement ne sont pas saisis : appariements contradictoires entre sources.
- **Classes** : Gladiator/Gladiateur (dps), Templar/Templier (tank), Assassin (dps), Ranger/Rôdeur (dps),
  Sorcerer/Sorcier (dps), Spiritmaster/Spiritualiste (dps), Cleric/Clerc (healer), Chanter/Aède (support) ;
  bornes par défaut min 0, max 10.
- **Rôles** : `tank`, `healer`, `dps`, `support`.
- **Instances** : `ludra`, `sanctuary`, `parties = 2`, `party_size = 5`, `cross_faction = true`,
  `duration_min = 60` `[ASSUMPTION]`, `room_label` court dérivé du nom. Aucune expédition tant que leurs noms
  ne sont pas relevés.
