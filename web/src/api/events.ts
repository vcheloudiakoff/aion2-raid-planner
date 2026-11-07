// Formulaires sous jeton (création CAP-1, candidature CAP-3) : jeton de formulaire et POST, avec la
// politique de nouvel essai d'EXPERIENCE.md (*Jeton de formulaire expiré*) : un jeton refusé est remplacé et la
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

export interface FormDeps {
  /** Nouveau jeton de la classe du formulaire. */
  renewToken: (formClass: FormClass) => Promise<string>;
  sleep: (ms: number) => Promise<void>;
}

export const defaultDeps: FormDeps = {
  renewToken: (formClass) => getFormToken(formClass),
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
};

export interface FormPost {
  url: string;
  formClass: FormClass;
  /** Secret du lien courant, en en-tête seulement. */
  linkSecret?: string;
}

/** POST d'un formulaire sous jeton, avec la politique de nouvel essai : jeton refusé = jeton neuf,
 * attente puis un seul nouvel essai ; trop rapide = attente puis un seul nouvel essai. `website` est
 * le honeypot, laissé vide par un humain. */
export async function postForm<T>(
  post: FormPost,
  input: object,
  token: string,
  website = "",
  deps: FormDeps = defaultDeps,
): Promise<T> {
  let renewed = false;
  let waited = false;
  for (;;) {
    try {
      return await postJson<T>(post.url, { ...input, formToken: token, website }, { linkSecret: post.linkSecret });
    } catch (e) {
      const code = e instanceof ApiError ? e.code : undefined;
      if (code === "form.token_invalid" && !renewed) {
        renewed = true;
        token = await deps.renewToken(post.formClass);
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

/** Publie l'événement. `website` est le honeypot, laissé vide par un humain. */
export function createEvent(
  input: CreateEvent,
  token: string,
  website = "",
  deps: FormDeps = defaultDeps,
): Promise<CreatedEvent> {
  return postForm<CreatedEvent>({ url: EVENTS_URL, formClass: "create" }, input, token, website, deps);
}
