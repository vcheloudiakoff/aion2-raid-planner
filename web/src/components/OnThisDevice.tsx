// « Sur cet appareil » (CAP-6, AD-3, EXPERIENCE.md *Liens et appareils*) : les raids organisés et les
// candidatures gardés dans `links.v1`, une ligne « Organisateur » vers `/o/` par lien organisateur et
// une ligne « Candidat » vers `/p/` par lien participant ; un lien direct seul n'a pas de ligne. Rien
// à montrer : la section est masquée.
//
// - Premier rendu depuis le stockage seul (AD-19) : le rôle, et un squelette à la place du titre.
// - Au montage, une lecture publique `GET /api/events/{id}` par événement distinct, sans secret :
//   aucun secret de `links.v1` ne part d'ici (AD-3), il ne sert qu'au lien de la ligne. Pas de
//   relecture.
// - Titre = instance et jour (`eventTitle`) ; un raid annulé le dit. Lecture en échec : ligne gardée,
//   sans titre. `event.not_found` : l'événement est oublié sur cet appareil, ses lignes disparaissent.
// - Une fois toutes les lectures finies, un seul tri : prochain créneau à venir (ou en cours)
//   d'abord, puis les raids passés ou annulés, le plus récent d'abord, puis les lectures en échec.
//   Ensuite, plus aucune ligne ne bouge.
// - « Oublier » retire la seule entrée locale de la ligne, après une confirmation propre à son rôle ;
//   rien n'est envoyé. Si le navigateur refuse l'écriture, la ligne reste et la feuille le dit.

import { useEffect, useId, useRef, useState, type RefObject } from "react";
import { Link } from "react-router";
import { getPublicEvent } from "../api/candidatures";
import { ApiError } from "../api/client";
import { forgetEvent, forgetOrganizer, forgetParticipant, readLinks, type Links } from "../api/links";
import type { PublicEvent, PublicSlot, ReferenceSet } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { ConfirmSheet } from "./ConfirmSheet";
import { eventTitle } from "./EventSummary";

export interface DeviceRow {
  /** `organizer:<eventId>` ou `participant:<eventId>:<candidatureId>`. */
  key: string;
  kind: "organizer" | "participant";
  eventId: string;
  candidatureId?: string;
  /** Chemin de la page de la ligne ; son secret n'y sert qu'à la navigation. */
  path: string;
}

/** Lecture publique d'un événement : absente tant qu'elle n'a pas répondu. */
type Read = { ok: true; event: PublicEvent } | { ok: false };

/** Lignes dans l'ordre du stockage : par événement, l'organisateur puis ses candidatures. */
export function deviceRows(links: Links): DeviceRow[] {
  const rows: DeviceRow[] = [];
  for (const [eventId, entry] of Object.entries(links)) {
    if (entry.organizer) {
      rows.push({ key: `organizer:${eventId}`, kind: "organizer", eventId, path: `/o/${entry.organizer}` });
    }
    for (const [candidatureId, secret] of Object.entries(entry.participants)) {
      rows.push({
        key: `participant:${eventId}:${candidatureId}`,
        kind: "participant",
        eventId,
        candidatureId,
        path: `/p/${secret}`,
      });
    }
  }
  return rows;
}

const slotEnd = (s: PublicSlot) => Date.parse(s.startsAt) + s.durationMin * 60_000;

/** Groupe de tri (0 à venir, 1 passé ou annulé, 2 lecture en échec) et clé dans le groupe : début du
 * prochain créneau, ou l'opposé du début du dernier pour mettre le plus récent d'abord. Un créneau en
 * cours compte comme à venir jusqu'à sa fin. */
function rank(read: Read | undefined, now: number): [number, number] {
  if (!read || !read.ok) return [2, 0];
  const slots = read.event.slots;
  if (read.event.status !== "cancelled") {
    const next = slots.find((s) => slotEnd(s) > now);
    if (next) return [0, Date.parse(next.startsAt)];
  }
  const last = slots[slots.length - 1];
  return [1, last ? -Date.parse(last.startsAt) : 0];
}

/** Confirmation de « Oublier » : par rôle, et selon qu'une autre ligne du même raid reste. */
const CONFIRM: Record<DeviceRow["kind"], { last: MessageKey; other: MessageKey }> = {
  organizer: { last: "home.device.confirm.organizer.last", other: "home.device.confirm.organizer.other" },
  participant: { last: "home.device.confirm.candidate.last", other: "home.device.confirm.candidate.other" },
};

/** Tri unique, stable : à égalité, l'ordre du stockage reste. */
export function sortDeviceRows(rows: DeviceRow[], reads: ReadonlyMap<string, Read>, now: number): DeviceRow[] {
  const keyed = rows.map((row) => ({ row, r: rank(reads.get(row.eventId), now) }));
  keyed.sort((a, b) => a.r[0] - b.r[0] || a.r[1] - b.r[1]);
  return keyed.map((k) => k.row);
}

interface Props {
  reference: ReferenceSet | null;
  /** Repli du focus quand la dernière ligne est oubliée et que la section disparaît. */
  fallbackFocus?: RefObject<HTMLElement | null>;
  now?: () => number;
}

export function OnThisDevice({ reference, fallbackFocus, now = Date.now }: Props) {
  const lang = useLang();
  const headingId = useId();
  const [rows, setRows] = useState(() => deviceRows(readLinks()));
  const [reads, setReads] = useState<ReadonlyMap<string, Read>>(() => new Map());
  const [confirming, setConfirming] = useState<DeviceRow | null>(null);
  /** Le navigateur a refusé l'écriture de « Oublier ». */
  const [forgetFailed, setForgetFailed] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const trigger = useRef<HTMLElement | null>(null);
  const fallbackRef = useRef(fallbackFocus);
  fallbackRef.current = fallbackFocus;
  /** Où rendre le focus si le bouton « Oublier » est parti avec sa ligne : le titre de la section,
   * ou le repli de la page si la section a disparu avec sa dernière ligne. Lu à la fermeture. */
  const [back] = useState<RefObject<HTMLElement | null>>(() => ({
    get current() {
      return heading.current?.isConnected ? heading.current : (fallbackRef.current?.current ?? null);
    },
  }));
  const nowRef = useRef(now);
  nowRef.current = now;

  useEffect(() => {
    // Au montage seulement : une lecture par événement, puis un seul tri.
    let alive = true;
    const ids = [...new Set(rows.map((r) => r.eventId))];
    if (ids.length === 0) return;
    const done = new Map<string, Read>();
    const read = (id: string) =>
      getPublicEvent(id).then(
        (event) => {
          done.set(id, { ok: true, event });
          if (alive) setReads((m) => new Map(m).set(id, { ok: true, event }));
        },
        (err: unknown) => {
          const code = err instanceof ApiError ? err.code : undefined;
          if (code === "event.not_found") {
            // L'événement n'existe plus : rien à garder sur cet appareil.
            forgetEvent(id);
            if (!alive) return;
            setRows((all) => all.filter((r) => r.eventId !== id));
            setConfirming((c) => (c && c.eventId === id ? null : c));
            return;
          }
          // Réseau, quota, panne : la ligne reste, sans titre, et rien n'est oublié.
          done.set(id, { ok: false });
          if (alive) setReads((m) => new Map(m).set(id, { ok: false }));
        },
      );
    void Promise.allSettled(ids.map(read)).then(() => {
      if (alive) setRows((all) => sortDeviceRows(all, done, nowRef.current()));
    });
    return () => {
      alive = false;
    };
    // Lignes du montage seulement : l'accueil ne se relit jamais.
  }, []);

  if (rows.length === 0) return null;

  const roleOf = (row: DeviceRow) => t(row.kind === "organizer" ? "home.device.organizer" : "home.device.candidate");

  function forget() {
    const row = confirming;
    if (!row) return;
    const done =
      row.kind === "organizer"
        ? forgetOrganizer(row.eventId)
        : row.candidatureId !== undefined && forgetParticipant(row.eventId, row.candidatureId);
    // Écriture refusée : l'entrée resterait, la ligne aussi.
    if (!done) {
      setForgetFailed(true);
      return;
    }
    setRows((all) => all.filter((r) => r.key !== row.key));
    setConfirming(null);
  }

  const alone = confirming !== null && !rows.some((r) => r.eventId === confirming.eventId && r.key !== confirming.key);

  return (
    <section data-testid="on-this-device" aria-labelledby={headingId} className="mt-8">
      <h2 ref={heading} id={headingId} tabIndex={-1} className="text-label text-ink-secondary outline-none">
        {t("home.device.title")}
      </h2>
      <ul className="mt-2 flex flex-col">
        {rows.map((row) => {
          const read = reads.get(row.eventId);
          const role = roleOf(row);
          const title = read?.ok ? eventTitle(read.event, reference, lang, now) : null;
          const cancelled = read?.ok === true && read.event.status === "cancelled";
          return (
            <li
              key={row.key}
              data-testid={`device-${row.key}`}
              aria-busy={read === undefined ? true : undefined}
              className="flex items-center gap-2 border-b border-border py-1"
            >
              <Link
                to={row.path}
                className="flex min-h-tap min-w-0 flex-1 flex-col justify-center py-1 text-ink-primary no-underline"
              >
                <span className="text-meta text-ink-secondary">
                  {role}
                  {cancelled && (
                    <>
                      {" · "}
                      <span className="text-danger">{t("event.cancelled")}</span>
                    </>
                  )}
                </span>
                {read === undefined ? (
                  <span aria-hidden="true" data-testid="skeleton" className="mt-1 block h-4 w-2/3 rounded-sm bg-surface-sunken" />
                ) : title !== null ? (
                  <span className="text-body break-words">{title}</span>
                ) : (
                  <span className="text-body text-ink-secondary">{t("home.device.unavailable")}</span>
                )}
              </Link>
              <button
                type="button"
                aria-label={t("home.device.forgetLabel", { name: title ? `${role} · ${title}` : role })}
                onClick={(e) => {
                  trigger.current = e.currentTarget;
                  setForgetFailed(false);
                  setConfirming(row);
                }}
                className="min-h-tap shrink-0 rounded-md border border-border px-3 text-label text-ink-secondary"
              >
                {t("home.device.forget")}
              </button>
            </li>
          );
        })}
      </ul>
      {confirming && (
        <ConfirmSheet
          message={t(CONFIRM[confirming.kind][alone ? "last" : "other"])}
          confirmLabel={t("home.device.forget")}
          cancelLabel={t("home.device.keep")}
          busyLabel={t("home.device.forget")}
          error={forgetFailed ? t("home.device.forgetFailed") : null}
          onConfirm={forget}
          onClose={() => setConfirming(null)}
          trigger={trigger}
          fallbackFocus={back}
        />
      )}
    </section>
  );
}
