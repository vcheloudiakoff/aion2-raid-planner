// Vue organisateur (CAP-5), sous le lien `/o/<secret>` : lecture et tri des candidatures. Le secret
// part en en-tête seulement, jamais dans l'URL ni le corps ; l'id de la candidature va dans le corps
// (AD-3). Pas de jeton de formulaire : le secret organisateur suffit (AD-7).

import { getJson, postJson } from "./client";
import type { Decision, OrganizerEvent, SetCandidatureStatus } from "./types";

export const ORGANIZER_URL = "/api/organizer";
export const STATUS_URL = `${ORGANIZER_URL}/candidature-status`;

export function getOrganizerView(secret: string): Promise<OrganizerEvent> {
  return getJson<OrganizerEvent>(ORGANIZER_URL, { linkSecret: secret });
}

/** Accepte ou refuse une candidature de l'événement ; rend la vue à jour. */
export function setCandidatureStatus(secret: string, candidatureId: string, status: Decision): Promise<OrganizerEvent> {
  const body: SetCandidatureStatus = { candidatureId, status };
  return postJson<OrganizerEvent>(STATUS_URL, body, { linkSecret: secret });
}
