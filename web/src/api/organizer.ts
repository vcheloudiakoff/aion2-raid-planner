// Vue organisateur (CAP-5), sous le lien `/o/<secret>` : lecture, tri des candidatures, réglages,
// créneaux, annulation, verrouillage, déverrouillage et placement. Le secret part en en-tête
// seulement, jamais dans l'URL ni le corps ; les ids (candidature, créneau, groupe) vont dans le corps
// (AD-3). Pas de jeton de formulaire : le secret
// organisateur suffit (AD-7). Chaque commande rend la vue à jour.

import { getJson, postJson } from "./client";
import type {
  ApplyPlacement,
  Decision,
  LockEvent,
  NewSlot,
  OrganizerEvent,
  PlacementEntry,
  SetCandidatureStatus,
  SlotRemoval,
  UpdateEventSettings,
} from "./types";

export const ORGANIZER_URL = "/api/organizer";
export const STATUS_URL = `${ORGANIZER_URL}/candidature-status`;
export const SETTINGS_URL = `${ORGANIZER_URL}/settings`;
export const SLOTS_URL = `${ORGANIZER_URL}/slots`;
export const SLOT_REMOVAL_URL = `${ORGANIZER_URL}/slot-removal`;
export const CANCELLATION_URL = `${ORGANIZER_URL}/cancellation`;
export const LOCK_URL = `${ORGANIZER_URL}/lock`;
export const UNLOCK_URL = `${ORGANIZER_URL}/unlock`;
export const PLACEMENT_URL = `${ORGANIZER_URL}/placement`;

export function getOrganizerView(secret: string): Promise<OrganizerEvent> {
  return getJson<OrganizerEvent>(ORGANIZER_URL, { linkSecret: secret });
}

/** Accepte ou refuse une candidature de l'événement ; rend la vue à jour. */
export function setCandidatureStatus(secret: string, candidatureId: string, status: Decision): Promise<OrganizerEvent> {
  const body: SetCandidatureStatus = { candidatureId, status };
  return postJson<OrganizerEvent>(STATUS_URL, body, { linkSecret: secret });
}

/** Remplace tous les réglages de l'événement. */
export function updateEventSettings(secret: string, body: UpdateEventSettings): Promise<OrganizerEvent> {
  return postJson<OrganizerEvent>(SETTINGS_URL, body, { linkSecret: secret });
}

/** Ajoute un créneau (événement `open` seulement). */
export function addSlot(secret: string, slot: NewSlot): Promise<OrganizerEvent> {
  return postJson<OrganizerEvent>(SLOTS_URL, slot, { linkSecret: secret });
}

/** Supprime un créneau (événement `open` seulement) et les disponibilités cochées dessus. */
export function removeSlot(secret: string, slotId: string): Promise<OrganizerEvent> {
  const body: SlotRemoval = { slotId };
  return postJson<OrganizerEvent>(SLOT_REMOVAL_URL, body, { linkSecret: secret });
}

/** Annule l'événement, pour tout le monde et sans retour. */
export function cancelEvent(secret: string): Promise<OrganizerEvent> {
  return postJson<OrganizerEvent>(CANCELLATION_URL, {}, { linkSecret: secret });
}

/** Verrouille l'événement : un groupe par entrée de `slotIds`, doublons répétés. */
export function lockEvent(secret: string, slotIds: string[]): Promise<OrganizerEvent> {
  const body: LockEvent = { slotIds };
  return postJson<OrganizerEvent>(LOCK_URL, body, { linkSecret: secret });
}

/** Ramène l'événement en préparation ; groupes et noms de salle sont gardés. */
export function unlockEvent(secret: string): Promise<OrganizerEvent> {
  return postJson<OrganizerEvent>(UNLOCK_URL, {}, { linkSecret: secret });
}

/** Remplace le placement complet des groupes retenus (événement `locked` seulement). */
export function applyPlacement(secret: string, placements: PlacementEntry[]): Promise<OrganizerEvent> {
  const body: ApplyPlacement = { placements };
  return postJson<OrganizerEvent>(PLACEMENT_URL, body, { linkSecret: secret });
}
