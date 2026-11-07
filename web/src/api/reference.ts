// Référence du jeu en cache `localStorage` (AD-19) : le premier rendu la lit sans attendre l'API,
// puis un GET conditionnel la rafraîchit. Clé versionnée : un changement de forme change la clé.

import { useCallback, useEffect, useRef, useState } from "react";
import { getConditional, remember } from "./client";
import type { Label, ReferenceSet } from "./types";
import { readJson, writeJson } from "../lib/storage";

export const REFERENCE_URL = "/api/reference";
export const REFERENCE_CACHE_KEY = "reference.v1";

export interface CachedReference {
  etag: string;
  data: ReferenceSet;
  /** Dernière confirmation par le serveur, ISO 8601 UTC. */
  fetchedAt: string;
}

function isCached(v: unknown): v is CachedReference {
  if (typeof v !== "object" || v === null) return false;
  const c = v as Record<string, unknown>;
  return (
    typeof c.etag === "string" &&
    typeof c.fetchedAt === "string" &&
    !Number.isNaN(Date.parse(c.fetchedAt)) &&
    isReferenceSet(c.data)
  );
}

function isReferenceSet(v: unknown): v is ReferenceSet {
  if (typeof v !== "object" || v === null) return false;
  const d = v as Record<string, unknown>;
  return (
    ["regions", "servers", "instances", "classes", "roles"].every((k) => Array.isArray(d[k])) &&
    typeof d.factions === "object" &&
    d.factions !== null &&
    !Array.isArray(d.factions)
  );
}

/** Libellé de référence dans la langue courante : `fr` avec repli sur `en` (AD-6). */
export function labelOf(label: Label, lang: string): string {
  return lang === "fr" ? (label.fr ?? label.en) : label.en;
}

/** Cache lu et mémoire conditionnelle amorcée ; `null` sans cache valide. */
export function loadCachedReference(): CachedReference | null {
  const v = readJson(REFERENCE_CACHE_KEY);
  if (!isCached(v)) return null;
  remember(REFERENCE_URL, v.etag, v.data);
  return v;
}

/** GET conditionnel ; met le cache à jour (heure comprise, même sur 304). */
export async function refreshReference(now: () => Date = () => new Date()): Promise<CachedReference> {
  const r = await getConditional<ReferenceSet>(REFERENCE_URL);
  const fresh: CachedReference = { etag: r.etag ?? "", data: r.body, fetchedAt: now().toISOString() };
  if (r.etag) writeJson(REFERENCE_CACHE_KEY, fresh);
  return fresh;
}

/** Référence pour l'écran : le cache tout de suite, puis le serveur ; nouvel essai au retour du
 * réseau et quand la page redevient visible. */
export function useReference(): CachedReference | null {
  const [ref, setRef] = useState<CachedReference | null>(loadCachedReference);
  // Appels qui se chevauchent : seule la réponse du dernier lancé est retenue.
  const seq = useRef(0);
  const sync = useCallback(() => {
    const mine = ++seq.current;
    refreshReference().then(
      (r) => {
        if (mine === seq.current) setRef(r);
      },
      () => {
        // Hors ligne ou erreur : on garde ce qu'on a ; le bandeau vient du client.
      },
    );
  }, []);
  useEffect(() => {
    sync();
    const onVisible = () => {
      if (document.visibilityState === "visible") sync();
    };
    window.addEventListener("online", sync);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("online", sync);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [sync]);
  return ref;
}
