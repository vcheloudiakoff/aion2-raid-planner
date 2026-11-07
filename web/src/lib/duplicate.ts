// « Dupliquer » (CAP-6, EXPERIENCE.md *Formulaires*) : un brouillon de création tiré de la vue
// organisateur enregistrée, jamais d'une saisie non enregistrée. Tout est repris sauf les dates :
// instance, région, portée, note, lien Discord, visibilité, CP minimum, bornes, et une ligne par
// créneau avec sa durée. Il passe à `/new` par l'état de navigation (`{ duplicate }`), sans route ni
// droit nouveau (AD-3) ; `/new` le relit par `duplicateFromState`, qui n'accepte qu'une forme valide.
// Aucune règle de référence ici : `/new` ignore une instance désactivée, ramène les bornes et ajuste
// la portée comme pour toute saisie.

import type { FactionScope, OrganizerEvent, ReferenceSet, Visibility } from "../api/types";
import { SLOTS_MAX, type Bound } from "./form";

export interface DuplicateDraft {
  instance: string;
  region: string;
  factionScope: FactionScope;
  note: string;
  /** Vide = aucun lien. */
  discordUrl: string;
  visibility: Visibility;
  minCp?: number;
  /** `classId → bornes`, celles du raid d'origine. */
  classBounds: Record<string, Bound>;
  /** Une ligne par créneau, sans date ; `durationMin` absente = durée de l'instance. */
  slots: { durationMin?: number }[];
}

/** Brouillon depuis la vue enregistrée. Une durée égale à celle de l'instance d'origine reste vide :
 * le créneau suivra la durée de l'instance choisie. Sans la référence, chaque durée est gardée. */
export function duplicateDraft(view: OrganizerEvent, reference: ReferenceSet | null): DuplicateDraft {
  const instanceDuration = reference?.instances.find((i) => i.id === view.event.instance)?.durationMin;
  return {
    instance: view.event.instance,
    region: view.event.region,
    factionScope: view.event.factionScope,
    note: view.event.note,
    discordUrl: view.discordUrl ?? "",
    visibility: view.event.visibility,
    ...(view.event.minCp !== undefined && { minCp: view.event.minCp }),
    classBounds: Object.fromEntries(Object.entries(view.classBounds).map(([id, b]) => [id, { min: b.min, max: b.max }])),
    slots: view.event.slots.map((s) => (s.durationMin === instanceDuration ? {} : { durationMin: s.durationMin })),
  };
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const isScope = (v: unknown): v is FactionScope => v === "all" || v === "elyos" || v === "asmodian";
const isVisibility = (v: unknown): v is Visibility => v === "listed" || v === "unlisted";

/** Brouillon porté par un état de navigation `{ duplicate }`, `null` s'il n'y en a pas ou s'il est
 * mal formé. Une borne invalide est ignorée ; les créneaux sont ramenés à 1..14 lignes. */
export function duplicateFromState(state: unknown): DuplicateDraft | null {
  if (!isRecord(state) || !isRecord(state.duplicate)) return null;
  const d = state.duplicate;
  if (
    typeof d.instance !== "string" ||
    typeof d.region !== "string" ||
    !isScope(d.factionScope) ||
    typeof d.note !== "string" ||
    typeof d.discordUrl !== "string" ||
    !isVisibility(d.visibility) ||
    (d.minCp !== undefined && !isCount(d.minCp)) ||
    !isRecord(d.classBounds) ||
    !Array.isArray(d.slots)
  ) {
    return null;
  }
  const classBounds: Record<string, Bound> = {};
  for (const [id, b] of Object.entries(d.classBounds)) {
    if (isRecord(b) && isCount(b.min) && isCount(b.max) && b.min <= b.max) classBounds[id] = { min: b.min, max: b.max };
  }
  const slots = d.slots
    .slice(0, SLOTS_MAX)
    .map((s) => (isRecord(s) && isCount(s.durationMin) ? { durationMin: s.durationMin } : {}));
  return {
    instance: d.instance,
    region: d.region,
    factionScope: d.factionScope,
    note: d.note,
    discordUrl: d.discordUrl,
    visibility: d.visibility,
    ...(d.minCp !== undefined && { minCp: d.minCp }),
    classBounds,
    slots: slots.length > 0 ? slots : [{}],
  };
}
