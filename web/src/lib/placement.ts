// Brouillon de composition (SPEC epic 3 CAP-3, EXPERIENCE.md *Écran de composition*) : le placement
// des groupes retenus vu par l'organisateur, ses gestes (placer, retirer, au banc, échanger), le
// corps d'`apply_placement` et le compte du bandeau « N candidatures ont changé depuis ton
// brouillon ». Fonctions pures, sans React : `OrganizerPage` garde le brouillon (il survit à un
// changement d'onglet) et l'onglet Composition, chargé à la demande, l'édite. Une candidature n'est
// jamais à deux places : chaque geste la retire d'abord de l'ancienne (l'interface déplace au lieu de
// dupliquer).

import type { OrganizerEvent, PlacementEntry } from "../api/types";

/** Placement d'un groupe : titulaires par slot (`"party.index"`), banc ordonné. */
export interface GroupPlacement {
  starters: Record<string, string>;
  bench: string[];
}

/** Placement des groupes retenus, par id de groupe, dans l'ordre des groupes. */
export type Placement = Record<string, GroupPlacement>;

/** Brouillon : la vue sur laquelle il a commencé et le placement édité. */
export interface Draft {
  base: OrganizerEvent;
  placement: Placement;
}

/** Une place : un slot de titulaire, ou un rang de banc (0 = premier ; `bench.length` = ajout à
 * la fin). */
export type Seat = { groupId: string; party: number; index: number } | { groupId: string; bench: number };

export const slotKey = (party: number, index: number) => `${party}.${index}`;

export const isBench = (s: Seat): s is { groupId: string; bench: number } => "bench" in s;

export const sameSeat = (a: Seat, b: Seat) =>
  a.groupId === b.groupId &&
  (isBench(a) ? isBench(b) && a.bench === b.bench : !isBench(b) && a.party === b.party && a.index === b.index);

/** Placement serveur des groupes retenus. */
export function placementOf(view: OrganizerEvent): Placement {
  const out: Placement = {};
  for (const g of view.groups) {
    if (!g.retained) continue;
    out[g.id] = {
      starters: Object.fromEntries(g.starters.map((s) => [slotKey(s.party, s.index), s.candidatureId])),
      bench: [...g.bench].sort((a, b) => a.benchIndex - b.benchIndex).map((b) => b.candidatureId),
    };
  }
  return out;
}

function clone(p: Placement): Placement {
  return Object.fromEntries(
    Object.entries(p).map(([id, g]) => [id, { starters: { ...g.starters }, bench: [...g.bench] }]),
  );
}

const groupOf = (p: Placement, id: string): GroupPlacement => (p[id] ??= { starters: {}, bench: [] });

/** Place actuelle d'une candidature, `null` si elle n'est dans aucun groupe retenu. */
export function seatOf(p: Placement, candidatureId: string): Seat | null {
  for (const [groupId, g] of Object.entries(p)) {
    for (const [key, id] of Object.entries(g.starters)) {
      if (id === candidatureId) {
        const [party, index] = key.split(".").map(Number);
        return { groupId, party, index };
      }
    }
    const bench = g.bench.indexOf(candidatureId);
    if (bench >= 0) return { groupId, bench };
  }
  return null;
}

/** Occupant d'une place, `null` si elle est libre. */
export function occupant(p: Placement, seat: Seat): string | null {
  const g = p[seat.groupId];
  if (!g) return null;
  return (isBench(seat) ? g.bench[seat.bench] : g.starters[slotKey(seat.party, seat.index)]) ?? null;
}

/** Retire la candidature de toute place (« Retirer »). */
export function unplace(p: Placement, candidatureId: string): Placement {
  const next = clone(p);
  for (const g of Object.values(next)) {
    for (const [key, id] of Object.entries(g.starters)) if (id === candidatureId) delete g.starters[key];
    g.bench = g.bench.filter((id) => id !== candidatureId);
  }
  return next;
}

/** Place la candidature sur une place libre, en la retirant d'abord de l'ancienne : un slot de
 * titulaire, ou la fin du banc. Un slot pris par une autre n'est jamais écrasé : rien ne change. */
export function placeAt(p: Placement, candidatureId: string, seat: Seat): Placement {
  const taken = isBench(seat) ? null : occupant(p, seat);
  if (taken && taken !== candidatureId) return p;
  const next = unplace(p, candidatureId);
  const g = groupOf(next, seat.groupId);
  if (isBench(seat)) g.bench.push(candidatureId);
  else g.starters[slotKey(seat.party, seat.index)] = candidatureId;
  return next;
}

/** « Au banc » : un titulaire passe à la fin du banc de son groupe. Sans effet s'il y est déjà. */
export function toBench(p: Placement, candidatureId: string): Placement {
  const seat = seatOf(p, candidatureId);
  if (!seat || isBench(seat)) return p;
  return placeAt(p, candidatureId, { groupId: seat.groupId, bench: Number.MAX_SAFE_INTEGER });
}

/** « Échanger » : les occupants des deux places sont permutés, d'un groupe à l'autre au besoin. Une
 * place libre reçoit l'occupant de l'autre, qui se vide (un rang de banc vidé disparaît, le banc
 * reste contigu). */
export function swapSeats(p: Placement, a: Seat, b: Seat): Placement {
  if (sameSeat(a, b)) return p;
  const next = clone(p);
  const [x, y] = [occupant(p, a), occupant(p, b)];
  const set = (seat: Seat, id: string | null) => {
    const g = groupOf(next, seat.groupId);
    if (isBench(seat)) {
      // Un rang au-delà du banc est un ajout à la fin ; `null` marque un rang à retirer.
      const at = Math.min(seat.bench, g.bench.length);
      (g.bench as (string | null)[])[at] = id;
    } else if (id) g.starters[slotKey(seat.party, seat.index)] = id;
    else delete g.starters[slotKey(seat.party, seat.index)];
  };
  set(a, y);
  set(b, x);
  for (const g of Object.values(next)) g.bench = g.bench.filter((id): id is string => id !== null);
  return next;
}

/** Corps d'`apply_placement` : le placement complet des groupes du brouillon, titulaires par équipe
 * puis slot, puis le banc dans l'ordre, rangs 1, 2, 3… */
export function bodyOf(p: Placement): PlacementEntry[] {
  return Object.entries(p).flatMap(([groupId, g]) => [
    ...Object.entries(g.starters)
      .map(([key, candidatureId]) => {
        const [party, index] = key.split(".").map(Number);
        return { candidatureId, groupId, party, index };
      })
      .sort((a, b) => a.party - b.party || a.index - b.index),
    ...g.bench.map((candidatureId, i) => ({ candidatureId, groupId, benchIndex: i + 1 })),
  ]);
}

/** Place serveur de chaque candidature, tous groupes : `groupe|s|équipe|slot` ou `groupe|b|rang`. */
function serverSeats(view: OrganizerEvent): Map<string, string> {
  const seats = new Map<string, string>();
  for (const g of view.groups) {
    for (const s of g.starters) seats.set(s.candidatureId, `${g.id}|s|${s.party}|${s.index}`);
    for (const b of g.bench) seats.set(b.candidatureId, `${g.id}|b|${b.benchIndex}`);
  }
  return seats;
}

/** Candidatures changées entre la vue de base du brouillon et la vue rafraîchie : ajoutées ou
 * disparues, ou dont le statut, la classe, le CP, les créneaux cochés ou la place enregistrée ont
 * changé. Chacune compte une fois. */
export function changedCount(base: OrganizerEvent, current: OrganizerEvent): number {
  const before = new Map(base.candidatures.map((c) => [c.id, c]));
  const after = new Map(current.candidatures.map((c) => [c.id, c]));
  const [seatsBefore, seatsAfter] = [serverSeats(base), serverSeats(current)];
  const slots = (c: OrganizerEvent["candidatures"][number]) =>
    c.availabilities
      .map((a) => a.slotId)
      .sort()
      .join(",");
  let changed = 0;
  for (const id of new Set([...before.keys(), ...after.keys()])) {
    const [b, a] = [before.get(id), after.get(id)];
    if (
      !b ||
      !a ||
      b.status !== a.status ||
      b.class !== a.class ||
      b.cp !== a.cp ||
      slots(b) !== slots(a) ||
      seatsBefore.get(id) !== seatsAfter.get(id)
    ) {
      changed += 1;
    }
  }
  return changed;
}
