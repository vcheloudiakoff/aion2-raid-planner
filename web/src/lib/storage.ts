// Seul accès bas niveau à `localStorage` : `links.ts` (1-6) et les autres modules passent par lui. Tout est
// enveloppé : stockage bloqué, plein ou absent (navigation privée, cookies refusés) = valeur absente ou
// écriture ignorée, la page marche.

function store(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export function readItem(key: string): string | null {
  try {
    return store()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

/** Vrai si la valeur est écrite. */
export function writeItem(key: string, value: string): boolean {
  try {
    const s = store();
    if (!s) return false;
    s.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export function removeItem(key: string): void {
  try {
    store()?.removeItem(key);
  } catch {
    // Rien à faire : la valeur restera, sans conséquence pour la page.
  }
}

/** JSON illisible ou absent = `null`. */
export function readJson(key: string): unknown {
  const raw = readItem(key);
  if (raw === null) return null;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return null;
  }
}

export function writeJson(key: string, value: unknown): boolean {
  let raw: string;
  try {
    raw = JSON.stringify(value);
  } catch {
    return false;
  }
  return writeItem(key, raw);
}

/** Dernier pseudo, serveur et classe utilisés sur cet appareil (EXPERIENCE.md *Formulaires*). Le CP
 * n'est jamais retenu. Écrit après une candidature réussie seulement. */
export const CANDIDATE_PREFS_KEY = "prefs.candidate";

export interface CandidatePrefs {
  pseudo?: string;
  server?: string;
  class?: string;
}

export function readCandidatePrefs(): CandidatePrefs {
  const raw = readJson(CANDIDATE_PREFS_KEY);
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return {};
  const out: CandidatePrefs = {};
  for (const k of ["pseudo", "server", "class"] as const) {
    const v = (raw as Record<string, unknown>)[k];
    if (typeof v === "string" && v.length <= 64) out[k] = v;
  }
  return out;
}

export function writeCandidatePrefs(prefs: Required<CandidatePrefs>): boolean {
  return writeJson(CANDIDATE_PREFS_KEY, { pseudo: prefs.pseudo, server: prefs.server, class: prefs.class });
}
