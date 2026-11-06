// Liens secrets gardés par ce navigateur (AD-3), sous le schéma versionné `links.v1` :
// `{ [eventId]: { organizer?, join?, participants: { [candidatureId]: secret } } }`. C'est la base de
// « Mes événements ». Tout passe par `lib/storage` : stockage absent ou cassé = aucun lien, écriture
// ignorée (`false`), jamais d'exception. Les données relues sont assainies : une entrée invalide est
// ignorée, les valides restent. Ce module ne fait aucune requête : l'appelant n'envoie au serveur que
// le secret du chemin courant.

import { readJson, writeJson } from "../lib/storage";

export const LINKS_KEY = "links.v1";

export const ID_LENGTH = 22;
export const SECRET_LENGTH = 43;

export interface EventLinks {
  organizer?: string;
  join?: string;
  /** Secret participant par `candidatureId`. */
  participants: Record<string, string>;
}

/** Liens par `eventId`. */
export type Links = Record<string, EventLinks>;

const BASE64URL = /^[A-Za-z0-9_-]+$/;

export function isPublicId(v: unknown): v is string {
  return typeof v === "string" && v.length === ID_LENGTH && BASE64URL.test(v);
}

export function isSecret(v: unknown): v is string {
  return typeof v === "string" && v.length === SECRET_LENGTH && BASE64URL.test(v);
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isEmpty(e: EventLinks): boolean {
  return e.organizer === undefined && e.join === undefined && Object.keys(e.participants).length === 0;
}

function sanitizeEvent(v: unknown): EventLinks | null {
  if (!isRecord(v)) return null;
  const out: EventLinks = { participants: {} };
  if (isSecret(v.organizer)) out.organizer = v.organizer;
  if (isSecret(v.join)) out.join = v.join;
  if (isRecord(v.participants)) {
    for (const [candidatureId, secret] of Object.entries(v.participants)) {
      if (isPublicId(candidatureId) && isSecret(secret)) out.participants[candidatureId] = secret;
    }
  }
  return isEmpty(out) ? null : out;
}

/** Tous les liens valides gardés ; `{}` si rien, si illisible ou si le stockage est indisponible. */
export function readLinks(): Links {
  const raw = readJson(LINKS_KEY);
  const links: Links = {};
  if (!isRecord(raw)) return links;
  for (const [eventId, entry] of Object.entries(raw)) {
    if (!isPublicId(eventId)) continue;
    const clean = sanitizeEvent(entry);
    if (clean) links[eventId] = clean;
  }
  return links;
}

/** Liens d'un événement, `undefined` si aucun. */
export function eventLinks(eventId: string): EventLinks | undefined {
  return isPublicId(eventId) ? readLinks()[eventId] : undefined;
}

function update(eventId: string, change: (entry: EventLinks) => void): boolean {
  if (!isPublicId(eventId)) return false;
  const links = readLinks();
  const entry = links[eventId] ?? { participants: {} };
  change(entry);
  if (isEmpty(entry)) delete links[eventId];
  else links[eventId] = entry;
  return writeJson(LINKS_KEY, links);
}

/** Garde les liens organisateur et candidature directe d'un événement, sans toucher au reste. */
export function saveOrganizerLinks(eventId: string, secrets: { organizer: string; join: string }): boolean {
  if (!isSecret(secrets.organizer) || !isSecret(secrets.join)) return false;
  return update(eventId, (entry) => {
    entry.organizer = secrets.organizer;
    entry.join = secrets.join;
  });
}

/** Garde le lien participant d'une candidature, sans toucher au reste. */
export function saveParticipantLink(eventId: string, candidatureId: string, secret: string): boolean {
  if (!isPublicId(candidatureId) || !isSecret(secret)) return false;
  return update(eventId, (entry) => {
    entry.participants[candidatureId] = secret;
  });
}

/** Oublie le lien participant d'une candidature ; l'événement disparaît s'il ne reste rien. */
export function forgetParticipant(eventId: string, candidatureId: string): boolean {
  return update(eventId, (entry) => {
    delete entry.participants[candidatureId];
  });
}

/** Oublie tous les liens d'un événement. */
export function forgetEvent(eventId: string): boolean {
  return update(eventId, (entry) => {
    delete entry.organizer;
    delete entry.join;
    entry.participants = {};
  });
}
