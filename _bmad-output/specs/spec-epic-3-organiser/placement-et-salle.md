# Placement, verrouillage et salle — règles de l'epic 3

## Validation d'`apply_placement`

Entrée : le placement complet de l'événement, liste `(candidatureId, groupId, party, index | benchIndex)`
(AD-4). Événement en `locked` seulement.

| Cas | Traitement |
| --- | --- |
| Candidature d'un autre événement, inconnue, ou non `accepted` | 400 `placement.candidature_not_placeable` |
| Même candidature deux fois | 400 `placement.duplicate_candidature` |
| Groupe d'un autre événement | 400 `placement.unknown_group` |
| `party` ∉ [1, parties] ou `index` ∉ [1, party_size] du groupe figé | 400 `placement.out_of_layout` |
| Deux entrées sur le même `(group, party, index)` ou le même `(group, benchIndex)` | 400 `placement.slot_taken` |
| Candidature non disponible au créneau du groupe | **permis**, avertissement `unavailable` |
| Compte d'une classe hors `[min, max]` de `class_bounds` dans un groupe | **permis**, avertissement `class_bound` (« 3 / max 2 », « 1 / min 2 ») |
| Groupe incomplet | permis, état « 8 / 10 · manque 1 Soin » |

Les avertissements ne sont pas stockés : `OrganizerEvent` les calcule à la lecture (`domain`), par groupe et
par slot. `benchIndex` est renuméroté de façon contiguë à l'écriture `[ASSUMPTION]`.

## Verrouillage

- `lock_event(slotIds[])` : un groupe par entrée, apparié par `(slot_id, rang d'occurrence)` (AD-4) ;
  refus si un créneau est passé (`slot.in_past`) ou n'appartient pas à l'événement.
- `unlock_event` : refus si aucun créneau de l'événement n'est à venir (`event.no_upcoming_slot`).
- En `locked`, les candidatures restent modifiables seulement par retrait (participant) ou statut
  (organisateur) ; une nouvelle candidature reste `unplaced`.

## Nom de salle

Nouvelle variable de configuration de cet epic, qui complète `configuration.md` de l'epic 1 :

| Variable | Obligatoire | Défaut | Règle |
| --- | --- | --- | --- |
| `ROOM_NAME_TEMPLATE` | non | `{label}-{code}` (sans CP, décision de Vincent du 2026-11-07) | jetons `{label}` (`room_label` figé) et `{code}` (4 caractères Crockford, aléatoire injecté) ; littéraux ASCII imprimables ; `{code}` exactement une fois ; longueur fixe du gabarit vérifiée au démarrage contre `room_label_max` + 5 `[ASSUMPTION]` |

Unicité par index unique sur `room_name` des groupes non purgés ; collision → nouveau `{code}`, cinq essais
puis 500. `regenerate_room_name(groupId)` : seule autre écriture du nom.

## Message Discord (gabarit dans les locales)

Ordre : ligne de titre (instance, `<t:UNIX:F>`), nom de salle, « Faction : All » si portée `all`, Équipe 1
puis Équipe 2 en `Pseudo[SRV]` avec rôle, banc ordonné, lien `PUBLIC_BASE_URL/e/<id>`. Un bloc par groupe,
groupes dans l'ordre des créneaux. Slot libre = « libre ». Jamais `/o`, `/j` ni `/p`.
