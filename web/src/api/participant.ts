// Ma candidature (CAP-4), sous le lien personnel `/p/<secret>` : lecture, modification et retrait.
// Le secret part en en-tête seulement, jamais dans l'URL ni le corps (AD-3). Pas de jeton de
// formulaire : le secret participant suffit (AD-7).

import { getJson, postJson } from "./client";
import type { ParticipantView, SubmitCandidature } from "./types";

export const PARTICIPANT_URL = "/api/participant";
export const UPDATE_URL = `${PARTICIPANT_URL}/candidature`;
export const WITHDRAW_URL = `${PARTICIPANT_URL}/withdrawal`;
export const TEAMS_URL = `${PARTICIPANT_URL}/teams`;

export function getParticipant(secret: string): Promise<ParticipantView> {
  return getJson<ParticipantView>(PARTICIPANT_URL, { linkSecret: secret });
}

/** Remplace tout le contenu ; le statut ne change pas. */
export function updateCandidature(secret: string, input: SubmitCandidature): Promise<ParticipantView> {
  return postJson<ParticipantView>(UPDATE_URL, input, { linkSecret: secret });
}

/** `pending` ou `accepted` → `withdrawn`, terminal. */
export function withdrawCandidature(secret: string): Promise<ParticipantView> {
  return postJson<ParticipantView>(WITHDRAW_URL, {}, { linkSecret: secret });
}

/** Équipes souhaitées, en `open` comme en `locked` ; rien d'autre ne change. */
export function setTeams(
  secret: string,
  input: { teams: number[]; preferredTeam?: number },
): Promise<ParticipantView> {
  return postJson<ParticipantView>(TEAMS_URL, input, { linkSecret: secret });
}
