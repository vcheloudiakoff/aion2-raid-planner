// Lecture publique et candidature (CAP-2, CAP-3). `/e/<id>` lit par l'id public ; `/j/<secret>` lit
// et candidate par le secret du lien direct, envoyé en en-tête et jamais dans l'URL (AD-3).

import { getJson } from "./client";
import { defaultDeps, postForm, type FormDeps } from "./events";
import type { CreatedCandidature, PublicEvent, SubmitCandidature } from "./types";

export const JOIN_URL = "/api/join";

export function publicEventUrl(eventId: string): string {
  return `/api/events/${encodeURIComponent(eventId)}`;
}

/** Page publique ou lien direct : ce qui identifie l'événement. */
export type EventAccess = { mode: "public"; eventId: string } | { mode: "join"; secret: string };

export function getPublicEvent(eventId: string): Promise<PublicEvent> {
  return getJson<PublicEvent>(publicEventUrl(eventId));
}

export function getJoinEvent(secret: string): Promise<PublicEvent> {
  return getJson<PublicEvent>(JOIN_URL, { linkSecret: secret });
}

export function getEvent(access: EventAccess): Promise<PublicEvent> {
  return access.mode === "public" ? getPublicEvent(access.eventId) : getJoinEvent(access.secret);
}

/** Dépose la candidature : `pending` par `/e/`, `accepted` par `/j/`. */
export function submitCandidature(
  access: EventAccess,
  input: SubmitCandidature,
  token: string,
  website = "",
  deps: FormDeps = defaultDeps,
): Promise<CreatedCandidature> {
  const post =
    access.mode === "public"
      ? { url: `${publicEventUrl(access.eventId)}/candidatures`, formClass: "candidature" as const }
      : { url: `${JOIN_URL}/candidatures`, formClass: "candidature" as const, linkSecret: access.secret };
  return postForm<CreatedCandidature>(post, input, token, website, deps);
}
