// Largeur disponible lue par `matchMedia` et `useSyncExternalStore` (modèle de `theme.ts`), sans
// `ResizeObserver`. La règle est en `em` : elle suit la loupe du navigateur (qui réduit le viewport
// CSS) et la taille de police choisie par l'utilisateur.

import { useCallback, useSyncExternalStore } from "react";

/** Largeur où la matrice tient : 22,5em, soit 360 px de viewport et 328 px utiles à 16 px. */
export const MATRIX_MEDIA = "(min-width: 22.5em)";

function query(media: string): MediaQueryList | null {
  try {
    return globalThis.matchMedia?.(media) ?? null;
  } catch {
    return null;
  }
}

/** Vrai tant que `media` correspond. Sans `matchMedia`, faux : la vue la plus étroite s'impose. */
export function useMedia(media: string): boolean {
  const subscribe = useCallback(
    (onChange: () => void) => {
      const list = query(media);
      list?.addEventListener?.("change", onChange);
      return () => list?.removeEventListener?.("change", onChange);
    },
    [media],
  );
  const read = useCallback(() => query(media)?.matches ?? false, [media]);
  return useSyncExternalStore(subscribe, read, () => false);
}
