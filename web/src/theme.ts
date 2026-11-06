// Thème (DESIGN.md) : sombre par défaut ; tant que rien n'est choisi, suit `prefers-color-scheme`.
// Appliqué par `data-theme` sur `<html>` ; sans JS, la feuille CSS suit déjà la préférence système.

import { useSyncExternalStore } from "react";
import { readItem, writeItem } from "./lib/storage";

export type Theme = "dark" | "light";
export const THEMES: readonly Theme[] = ["dark", "light"];
export const THEME_KEY = "prefs.theme";

const LIGHT_QUERY = "(prefers-color-scheme: light)";

const isTheme = (v: unknown): v is Theme => v === "dark" || v === "light";

function lightQuery(): MediaQueryList | null {
  try {
    return globalThis.matchMedia?.(LIGHT_QUERY) ?? null;
  } catch {
    return null;
  }
}

/** Préférence système : clair seulement si le système le demande. */
export function systemTheme(): Theme {
  return lightQuery()?.matches ? "light" : "dark";
}

export function storedTheme(): Theme | null {
  const v = readItem(THEME_KEY);
  return isTheme(v) ? v : null;
}

let choice: Theme | null = storedTheme();
let resolved: Theme = choice ?? systemTheme();
const listeners = new Set<() => void>();

function apply() {
  if (typeof document !== "undefined") document.documentElement.dataset.theme = resolved;
}

function update() {
  const next = choice ?? systemTheme();
  if (next !== resolved) {
    resolved = next;
    apply();
    for (const l of listeners) l();
  } else {
    apply();
  }
}

apply();
lightQuery()?.addEventListener?.("change", update);

export function getTheme(): Theme {
  return resolved;
}

/** Choix explicite, mémorisé si possible ; il prend le pas sur le système. */
export function setTheme(theme: Theme): void {
  choice = theme;
  writeItem(THEME_KEY, theme);
  update();
}

export function subscribeTheme(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useTheme(): Theme {
  return useSyncExternalStore(subscribeTheme, getTheme, getTheme);
}

/** Réinitialise depuis le stockage et le système (tests). */
export function resetThemeForTests(): void {
  choice = storedTheme();
  update();
}
