// Seul client HTTP de la SPA (AD-5). Conditionnel manuel : l'API est `no-store`, le client garde
// `etag` + corps par URL et envoie `If-None-Match` ; un 304 rend le corps gardé. Chaque réponse
// porte `X-App-Build` : un build différent du premier vu lève le drapeau « nouvelle version », sans
// jamais recharger la page. Un `fetch` rejeté lève le drapeau « hors ligne », la réponse suivante le
// baisse. Le client n'envoie que le secret de lien qu'on lui passe, en en-tête.

import { useSyncExternalStore } from "react";
import type { Problem } from "./types";

/** Erreur typée : `status` 0 = serveur injoignable ; `code` = code du registre si problem+json. */
export class ApiError extends Error {
  readonly status: number;
  readonly code?: string;

  constructor(status: number, code?: string) {
    super(code ?? (status === 0 ? "unreachable" : `HTTP ${status}`));
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

export interface Conditional<T> {
  body: T;
  etag: string | null;
  /** Vrai si le serveur a répondu 304 et que le corps vient de la mémoire. */
  notModified: boolean;
}

export interface ClientStatus {
  offline: boolean;
  newBuild: boolean;
}

/** Au-delà, une requête sans réponse compte comme serveur injoignable. */
export const TIMEOUT_MS = 10_000;

/** Statuts renvoyés par l'edge quand le serveur derrière lui ne répond pas. */
const GATEWAY_DOWN = new Set([502, 504]);

const memo = new Map<string, { etag: string; body: unknown }>();
let firstBuild: string | null = null;
let status: ClientStatus = { offline: false, newBuild: false };
const listeners = new Set<() => void>();

function setStatus(patch: Partial<ClientStatus>) {
  const next = { ...status, ...patch };
  if (next.offline === status.offline && next.newBuild === status.newBuild) return;
  status = next;
  for (const l of listeners) l();
}

function noteBuild(build: string | null) {
  if (!build) return;
  if (firstBuild === null) firstBuild = build;
  else if (build !== firstBuild) setStatus({ newBuild: true });
}

export function getStatus(): ClientStatus {
  return status;
}

export function subscribeStatus(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useClientStatus(): ClientStatus {
  return useSyncExternalStore(subscribeStatus, getStatus, getStatus);
}

/** Amorce la mémoire conditionnelle (cache `localStorage` relu au démarrage). */
export function remember(url: string, etag: string, body: unknown): void {
  memo.set(url, { etag, body });
}

async function problemCode(res: Response): Promise<string | undefined> {
  if (!(res.headers.get("Content-Type") ?? "").startsWith("application/problem+json")) return undefined;
  try {
    const p = (await res.json()) as Partial<Problem>;
    return typeof p.code === "string" ? p.code : undefined;
  } catch {
    return undefined;
  }
}

export interface RequestOptions {
  /** Secret de lien du chemin courant, envoyé dans `X-Link-Secret` et jamais dans l'URL (AD-3). */
  linkSecret?: string;
}

/** GET conditionnel : corps, `ETag` et provenance. */
export async function getConditional<T>(url: string, options: RequestOptions = {}): Promise<Conditional<T>> {
  const headers: Record<string, string> = { Accept: "application/json" };
  // Secret vide = absent : le serveur répond 401 `auth.missing`, pas 404.
  const secret = options.linkSecret || undefined;
  if (secret) headers["X-Link-Secret"] = secret;
  // Mémoire par URL et secret : deux droits sur une même URL ne partagent jamais un corps.
  const key = secret ? `${url}\u0000${secret}` : url;
  const known = memo.get(key);
  if (known) headers["If-None-Match"] = known.etag;
  let res: Response;
  try {
    res = await fetch(url, { headers, cache: "no-store", signal: AbortSignal.timeout(TIMEOUT_MS) });
  } catch {
    setStatus({ offline: true });
    throw new ApiError(0);
  }
  noteBuild(res.headers.get("X-App-Build"));
  if (GATEWAY_DOWN.has(res.status)) {
    setStatus({ offline: true });
    throw new ApiError(0);
  }
  setStatus({ offline: false });
  if (res.status === 304) {
    if (!known) throw new ApiError(304);
    return { body: known.body as T, etag: known.etag, notModified: true };
  }
  if (!res.ok) throw new ApiError(res.status, await problemCode(res));
  let body: T;
  try {
    body = (await res.json()) as T;
  } catch {
    // 2xx au corps vide ou non JSON : erreur typée, jamais un `SyntaxError` brut.
    throw new ApiError(res.status);
  }
  const etag = res.headers.get("ETag");
  if (etag) memo.set(key, { etag, body });
  else memo.delete(key);
  return { body, etag, notModified: false };
}

export async function getJson<T>(url: string, options: RequestOptions = {}): Promise<T> {
  return (await getConditional<T>(url, options)).body;
}

/** Remise à zéro (tests). */
export function resetClientForTests(): void {
  memo.clear();
  firstBuild = null;
  status = { offline: false, newBuild: false };
  for (const l of listeners) l();
}
