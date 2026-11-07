// Conversions de fuseau (AD-11) : le navigateur saisit en heure locale, l'API ne voit que l'UTC sous la
// forme unique `YYYY-MM-DDTHH:MM:SSZ`.

const LOCAL_INPUT = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?$/;

/** Valeur d'un `<input type="datetime-local">` (heure locale) → instant UTC canonique ; `null` si la
 * valeur est vide, mal formée ou n'existe pas dans le fuseau (jour ou heure hors bornes). */
export function localInputToUtc(value: string): string | null {
  const m = LOCAL_INPUT.exec(value);
  if (!m) return null;
  const [y, mo, d, h, mi, s] = m.slice(1).map((v) => Number(v ?? 0));
  const date = new Date(y, mo - 1, d, h, mi, s);
  if (Number.isNaN(date.getTime()) || date.getFullYear() !== y || date.getMonth() !== mo - 1 || date.getDate() !== d) {
    return null;
  }
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** Fuseau du navigateur, à afficher une fois en tête d'écran : « Europe/Paris (UTC+1) ». */
export function timeZoneLabel(lang: string, at: Date = new Date()): string {
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const short = new Intl.DateTimeFormat(lang, { timeZoneName: "short" })
    .formatToParts(at)
    .find((p) => p.type === "timeZoneName")?.value;
  if (!zone) return short ?? "";
  return short && short !== zone ? `${zone} (${short})` : zone;
}
