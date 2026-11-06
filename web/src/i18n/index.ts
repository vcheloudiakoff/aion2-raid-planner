// `t()` maison (AD-12) : deux catalogues plats, mêmes clés, langue courante dans un petit magasin
// observé par React. Langue = choix mémorisé, sinon `navigator.language`, sinon français.

import { useSyncExternalStore } from "react";
import en from "../locales/en.json";
import fr from "../locales/fr.json";
import { readItem, writeItem } from "../lib/storage";

export type Lang = "fr" | "en";
export const LANGS: readonly Lang[] = ["fr", "en"];
export type MessageKey = keyof typeof fr;
export type Vars = Record<string, string | number>;

export const LANG_KEY = "prefs.lang";

const catalogs: Record<Lang, Record<string, string>> = { fr, en };

const isLang = (v: unknown): v is Lang => v === "fr" || v === "en";

/** Langue d'après une étiquette BCP 47 (`en-US` → `en`) ; inconnue ou absente → `fr`. */
export function detectLang(tag: string | null | undefined): Lang {
  const primary = (tag ?? "").trim().toLowerCase().split(/[-_]/)[0];
  return isLang(primary) ? primary : "fr";
}

/** Choix mémorisé, sinon langue du navigateur. */
export function initialLang(): Lang {
  const stored = readItem(LANG_KEY);
  if (isLang(stored)) return stored;
  return detectLang(globalThis.navigator?.language);
}

/** Texte d'une clé dans une langue, `{nom}` remplacé par `vars.nom`. Clé absente → la clé elle-même. */
export function translate(lang: Lang, key: MessageKey | string, vars?: Vars): string {
  const text = catalogs[lang][key] ?? catalogs.fr[key] ?? key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    Object.hasOwn(vars, name) ? String(vars[name]) : whole,
  );
}

/** Vrai si la clé existe (codes d'erreur venus du serveur). */
export function hasKey(key: string): boolean {
  return Object.hasOwn(catalogs.fr, key);
}

let current: Lang = initialLang();
const listeners = new Set<() => void>();

function applyDocumentLang(lang: Lang) {
  if (typeof document !== "undefined") document.documentElement.lang = lang;
}
applyDocumentLang(current);

export function getLang(): Lang {
  return current;
}

/** Change la langue, la mémorise si possible et met à jour `<html lang>`. */
export function setLang(lang: Lang): void {
  writeItem(LANG_KEY, lang);
  if (lang === current) return;
  current = lang;
  applyDocumentLang(lang);
  for (const l of listeners) l();
}

export function subscribeLang(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Texte dans la langue courante. */
export function t(key: MessageKey, vars?: Vars): string {
  return translate(current, key, vars);
}

/** Langue courante ; le composant se re-rend quand elle change. */
export function useLang(): Lang {
  return useSyncExternalStore(subscribeLang, getLang, getLang);
}

/** Réinitialise depuis le stockage et le navigateur (tests). */
export function resetLangForTests(): void {
  current = initialLang();
  applyDocumentLang(current);
  for (const l of listeners) l();
}
