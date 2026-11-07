// Outils des formulaires de l'événement, partagés par la création (`/new`) et les réglages (`/o`) :
// bornes de saisie, texte d'un code d'erreur, entiers stricts, bornes par classe ramenées à la taille
// de l'instance, et erreurs de champ du serveur affichées sous leur champ.

import { useState, type Dispatch, type ReactNode, type SetStateAction } from "react";
import type { Instance } from "../api/types";
import { hasKey, t, type MessageKey } from "../i18n";

export const NOTE_MAX = 280;
export const SLOTS_MAX = 14;
/** Durée d'un créneau en minutes, bornes du serveur. */
export const DURATION_MIN = 15;
export const DURATION_MAX = 720;

export interface Bound {
  min: number;
  max: number;
}

/** Codes d'erreur du serveur par chemin JSON (`slots.2.startsAt`). */
export type Fields = Record<string, string>;

/** Texte d'un code d'erreur du serveur, repli générique si la locale ne le connaît pas. */
export function errorText(code: string | undefined): string {
  const key = `errors.${code}`;
  return code && hasKey(key) ? t(key as MessageKey) : t("common.error");
}

/** Longueur en caractères, comme le serveur la compte. */
export const chars = (s: string) => [...s].length;

/** Champ numérique vide ou entier sûr : jamais de décimale ni d'exposant envoyé au serveur. */
export const isIntOrEmpty = (s: string) => {
  const v = s.trim();
  return v === "" || (/^-?\d+$/.test(v) && Number.isSafeInteger(Number(v)));
};

/** Joueurs que prend une instance. */
export const capOf = (instance: Instance) => instance.parties * instance.partySize;

/** Bornes par défaut d'une classe, ramenées à la taille de l'instance (comme le serveur). */
export function defaultBound(defaultMin: number, defaultMax: number, cap: number): Bound {
  const max = Math.min(defaultMax, cap);
  return { min: Math.min(defaultMin, max), max };
}

/** Bornes ramenées à la taille d'une autre instance. */
export function clampBounds(all: Record<string, Bound>, cap: number): Record<string, Bound> {
  return Object.fromEntries(Object.entries(all).map(([id, b]) => [id, defaultBound(b.min, b.max, cap)]));
}

/** Vrai si `key` est `path` ou un chemin sous `path`. */
const under = (key: string, path: string) => key === path || key.startsWith(`${path}.`);

export interface FieldErrors {
  fields: Fields;
  setFields: Dispatch<SetStateAction<Fields>>;
  errorId: (path: string) => string;
  /** Ligne d'erreur sous un champ ; `nested` prend aussi les chemins sous `path`. */
  errorLine: (path: string, nested?: boolean) => ReactNode;
  /** Vrai si `path` ou un chemin sous lui porte une erreur. */
  hasNested: (path: string) => boolean;
  /** Attributs ARIA d'un champ en erreur. */
  invalid: (path: string) => { "aria-invalid"?: true; "aria-describedby"?: string };
  /** Retire les erreurs dont le chemin est `path` ou commence par `path.`. */
  clearField: (path: string) => void;
}

/** Erreurs de champ rendues par le serveur, affichées sous leur champ jusqu'à ce qu'il change. */
export function useFieldErrors(idPrefix: string): FieldErrors {
  const [fields, setFields] = useState<Fields>({});
  const errorId = (path: string) => `${idPrefix}-err-${path}`;
  return {
    fields,
    setFields,
    errorId,
    errorLine: (path, nested = false) => {
      const code = nested ? Object.entries(fields).find(([k]) => under(k, path))?.[1] : fields[path];
      if (!code) return null;
      return (
        <p id={errorId(path)} className="mt-1 text-meta text-danger">
          {errorText(code)}
        </p>
      );
    },
    hasNested: (path) => Object.keys(fields).some((k) => under(k, path)),
    invalid: (path) => (fields[path] ? { "aria-invalid": true, "aria-describedby": errorId(path) } : {}),
    clearField: (path) =>
      setFields((all) => {
        const keys = Object.keys(all).filter((k) => under(k, path));
        if (keys.length === 0) return all;
        const next = { ...all };
        for (const k of keys) delete next[k];
        return next;
      }),
  };
}
