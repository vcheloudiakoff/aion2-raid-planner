// Création d'un événement (CAP-1) : jeton de formulaire et `POST /api/events`, avec la politique de
// nouvel essai d'EXPERIENCE.md (*Jeton de formulaire expiré*) : un jeton refusé est remplacé et la
// requête rejouée une fois, sans rien dire ; un envoi trop rapide attend `MIN_SUBMIT_DELAY_MS` puis
// rejoue une fois. Toute autre erreur remonte telle quelle à l'écran.

import { ApiError, getJson, postJson } from "./client";
import type { CreateEvent, CreatedEvent, FormToken } from "./types";

export const EVENTS_URL = "/api/events";
export const FORM_TOKEN_URL = "/api/form-token";

/** Âge minimal d'un jeton accepté par le serveur (`MIN_SUBMIT_DELAY_MS`, 3 s par défaut). */
export const MIN_SUBMIT_DELAY_MS = 3000;

export type FormClass = "create" | "candidature";

export function getFormToken(formClass: FormClass): Promise<string> {
  return getJson<FormToken>(`${FORM_TOKEN_URL}?class=${formClass}`).then((t) => t.token);
}

export interface CreateDeps {
  /** Nouveau jeton `create`. */
  renewToken: () => Promise<string>;
  sleep: (ms: number) => Promise<void>;
}

export const defaultDeps: CreateDeps = {
  renewToken: () => getFormToken("create"),
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
};

/** Publie l'événement. `website` est le honeypot, laissé vide par un humain. */
export async function createEvent(
  input: CreateEvent,
  token: string,
  website = "",
  deps: CreateDeps = defaultDeps,
): Promise<CreatedEvent> {
  let renewed = false;
  let waited = false;
  for (;;) {
    try {
      return await postJson<CreatedEvent>(EVENTS_URL, { ...input, formToken: token, website });
    } catch (e) {
      const code = e instanceof ApiError ? e.code : undefined;
      if (code === "form.token_invalid" && !renewed) {
        renewed = true;
        token = await deps.renewToken();
        // Un jeton neuf est trop jeune pour le serveur : attendre avant de rejouer.
        await deps.sleep(MIN_SUBMIT_DELAY_MS);
        waited = true;
        continue;
      }
      if (code === "form.too_fast" && !waited) {
        waited = true;
        await deps.sleep(MIN_SUBMIT_DELAY_MS);
        continue;
      }
      throw e;
    }
  }
}
