# Commandes, requêtes et droits de l'epic 2

Chaque écriture est une commande `app::commands::*` (AD-16) ; chaque lecture une requête par droit qui
renvoie le DTO du droit (AD-5). Chemins d'API indicatifs `[ASSUMPTION]` : la règle fixe est `/api/<noms
pluriels>` en kebab-case, secret dans `X-Link-Secret`.

| Geste | Commande / requête | Droit | Classe de limiteur | DTO renvoyé | Erreurs nommées |
| --- | --- | --- | --- | --- | --- |
| Créer l'événement | `create_event` | public + jeton de formulaire | `create` | `CreatedEvent` (ids + 3 liens, une seule fois) | `form.*`, `reference.unknown`, `slot.in_past`, `limit.*` |
| Lire l'événement public | `public_event` | public (`/e` id) | `read` | `PublicEvent` | `event.not_found` |
| Lire le formulaire direct | `public_event` via secret `/j` | direct | `read` | `PublicEvent` | `auth.*` |
| Candidater | `submit_candidature` | public (`/e`) ou direct (`/j`) + jeton | `candidature` | `CreatedCandidature` (id + lien `/p`) | `candidature.server_out_of_region`, `candidature.faction_out_of_scope`, `candidature.preference_not_available`, `event.closed` |
| Lire sa candidature | `participant_view` | participant | `read` | `ParticipantView` | `auth.*` |
| Modifier sa candidature | `update_candidature` | participant | `write` | `ParticipantView` | `event.locked`, `candidature.withdrawn` |
| Se retirer | `withdraw_candidature` | participant | `write` | `ParticipantView` | `candidature.withdrawn` |
| Lire l'organisateur | `organizer_view` | organisateur | `read` | `OrganizerEvent` | `auth.*` |
| Accepter / refuser | `set_candidature_status` | organisateur | `write` | `OrganizerEvent` | `candidature.transition_forbidden` |
| Régler l'événement | `update_event_settings` | organisateur | `write` | `OrganizerEvent` | `event.field_immutable`, `slot.in_past` |
| Ajouter / supprimer un créneau | `add_slot`, `remove_slot` | organisateur | `write` | `OrganizerEvent` | `event.locked` |
| Annuler | `cancel_event` | organisateur | `write` | `OrganizerEvent` | `event.cancelled` |

Les codes d'erreur sont des exemples de la forme `<entité>.<cas>` ; la liste fermée d'entités d'AD-5 reste
la seule permise.

## Contenu des DTO (au-delà d'AD-4 *Données exposées*)

- `PublicEvent` : `id`, `instance`, `region`, `factionScope`, `visibility`, `status`, `minCp`, `note`
  (≤ 280 caractères, entière : c'est la « note courte » d'AD-4), `slots` (`id`, `startsAt`, `durationMin`),
  `countsByClass`, `countsByRole`, `missingRoles` ; jamais `discordUrl` ni candidatures.
- `OrganizerEvent` : tout `PublicEvent` + `discordUrl`, `classBounds`, `candidatures` (id, pseudo, serveur,
  `pseudoSrv` composé côté serveur, classe, rôle, CP, statut, `origin`, disponibilités avec préférence,
  `duplicate`), `slot.hasGroups`.
- `ParticipantView` : sa candidature, `status`, l'événement public, `discordUrl` si `accepted` ; placement,
  salle et confirmation s'ajoutent en epics 3 et 5.
