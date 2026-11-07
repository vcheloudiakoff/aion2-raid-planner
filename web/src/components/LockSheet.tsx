// Feuille Verrouiller de l'onglet Disponibilités (SPEC epic 3 CAP-2, EXPERIENCE.md *Grille joueurs ×
// créneaux*, AD-4 *Verrouillage*) : une ligne par créneau à venir, avec sa case « Retenir », un
// Stepper du nombre de groupes et « Assez pour N groupes » (N = disponibles parmi les `accepted`
// divisés par la taille de groupe de l'instance, 0 compris). Pré-cochage : les groupes retenus du
// dernier verrouillage, sinon le créneau qui a le plus de disponibles (le plus tôt à égalité), à
// 1 groupe ; verrouiller tient donc en deux touchers. « Verrouiller K groupes » envoie un id par
// groupe, doublons répétés, dans l'ordre des créneaux ; désactivé avec sa raison quand K = 0 ou
// K > `GROUPS_MAX`. Un créneau que le serveur dit commencé (`slot.in_past`) quitte la feuille, avec
// l'erreur. Le serveur ne vérifie pas les disponibilités : la borne du Stepper n'est qu'un garde-fou
// de saisie.

import { useId, useRef, useState, type RefObject } from "react";
import type { OrganizerEvent, PublicSlot, ReferenceSet } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { GROUPS_MAX, type Fields } from "../lib/form";
import { formatSlot } from "../lib/time";
import { Sheet } from "./Sheet";
import { Stepper } from "./Stepper";

/** Joueurs par groupe (`parties × partySize`) de l'instance de l'événement, retirée comprise ;
 * `null` si la référence ne la connaît pas (ou pas encore). */
export function groupSize(view: OrganizerEvent, reference: ReferenceSet | null): number | null {
  const instance = reference?.instances.find((i) => i.id === view.event.instance);
  return instance ? instance.parties * instance.partySize : null;
}

/** Groupes complets que les disponibles d'un créneau permettent. */
export function enoughFor(availableAccepted: number, size: number | null): number {
  return size && size > 0 ? Math.floor(availableAccepted / size) : 0;
}

/** Créneaux à venir, dans l'ordre de l'événement. */
export function upcomingSlots(view: OrganizerEvent, now: number): PublicSlot[] {
  return view.event.slots.filter((s) => Date.parse(s.startsAt) > now);
}

/** Pré-cochage : `slotId → groupes`. Les groupes retenus du dernier verrouillage sur des créneaux
 * encore à venir ; sinon le créneau à venir qui a le plus de disponibles, le plus tôt à égalité,
 * à 1 groupe ; rien s'il n'y a aucun créneau à venir. */
export function initialPicks(view: OrganizerEvent, now: number): Record<string, number> {
  const upcoming = upcomingSlots(view, now);
  const ids = new Set(upcoming.map((s) => s.id));
  const picks: Record<string, number> = {};
  for (const g of view.groups) {
    if (g.retained && ids.has(g.slotId)) picks[g.slotId] = (picks[g.slotId] ?? 0) + 1;
  }
  if (Object.keys(picks).length > 0) return picks;
  const available = (id: string) => view.slots.find((s) => s.slotId === id)?.availableAccepted ?? 0;
  let best: PublicSlot | null = null;
  for (const s of upcoming) if (!best || available(s.id) > available(best.id)) best = s;
  return best ? { [best.id]: 1 } : {};
}

/** Corps de `lock_event` : un id par groupe, doublons répétés, dans l'ordre des créneaux. */
export function lockList(slots: readonly PublicSlot[], rows: Readonly<Record<string, Row>>): string[] {
  return slots.flatMap((s) => {
    const r = rows[s.id];
    return r?.on ? Array<string>(r.count).fill(s.id) : [];
  });
}

/** Clé au singulier ou au pluriel selon la langue (0 est au singulier en français). */
function plural(lang: string, count: number, one: MessageKey, other: MessageKey): MessageKey {
  return new Intl.PluralRules(lang).select(count) === "one" ? one : other;
}

interface Row {
  on: boolean;
  count: number;
}

interface Props {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  now: () => number;
  /** Verrouillage en vol. */
  busy: boolean;
  /** Erreur du dernier essai, déjà traduite. */
  error: string | null;
  /** Envoie la liste ; rend les erreurs de champ d'un refus (`slotIds.N`), vide sinon. */
  onLock: (slotIds: string[]) => Promise<Fields>;
  onClose: () => void;
  trigger: RefObject<HTMLElement | null>;
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function LockSheet({ view, reference, now, busy, error, onLock, onClose, trigger, fallbackFocus }: Props) {
  const lang = useLang();
  const ids = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  // Les créneaux et le pré-cochage sont figés à l'ouverture : une relecture ne décoche rien.
  const [slots, setSlots] = useState(() => upcomingSlots(view, now()));
  const [initial] = useState(() => initialPicks(view, now()));
  const [rows, setRows] = useState<Record<string, Row>>(() =>
    Object.fromEntries(slots.map((s) => [s.id, { on: s.id in initial, count: initial[s.id] ?? 1 }])),
  );
  const size = groupSize(view, reference);
  const available = (id: string) => view.slots.find((s) => s.slotId === id)?.availableAccepted ?? 0;
  const total = lockList(slots, rows).length;
  const reason: MessageKey | null = total === 0 ? "lock.reason.none" : total > GROUPS_MAX ? "lock.reason.max" : null;
  const reasonId = `${ids}-reason`;
  const close = () => {
    if (!busy) onClose();
  };
  const set = (id: string, patch: Partial<Row>) =>
    setRows((r) => ({ ...r, [id]: { ...(r[id] ?? { on: false, count: 1 }), ...patch } }));

  async function submit() {
    const list = lockList(slots, rows);
    const fields = await onLock(list);
    // Créneaux commencés entre-temps : ils quittent la feuille, l'erreur reste affichée.
    const past = new Set(
      Object.entries(fields)
        .filter(([path, code]) => code === "slot.in_past" && path.startsWith("slotIds."))
        .map(([path]) => list[Number(path.slice("slotIds.".length))])
        .filter((id): id is string => id !== undefined),
    );
    if (past.size > 0) setSlots(upcomingSlots(view, now()).filter((s) => !past.has(s.id)));
  }

  return (
    <Sheet labelledBy={`${ids}-title`} onClose={close} trigger={trigger} initialFocus={heading} fallbackFocus={fallbackFocus}>
      <div data-testid="lock-sheet">
        <h2 id={`${ids}-title`} ref={heading} tabIndex={-1} className="text-title outline-none">
          {t("lock.title")}
        </h2>
        <p className="mt-1 text-meta text-ink-secondary">{t("lock.hint")}</p>
        <ul className="mt-3 flex flex-col">
          {slots.map((s) => {
            const name = formatSlot(s.startsAt, lang);
            const enough = enoughFor(available(s.id), size);
            // Une borne sous le pré-cochage ne retire pas un groupe déjà retenu.
            const max = Math.max(1, enough, initial[s.id] ?? 1);
            const row = rows[s.id] ?? { on: false, count: 1 };
            return (
              <li
                key={s.id}
                data-testid={`lock-row-${s.id}`}
                className="flex items-center gap-2 border-b border-border py-1"
              >
                <label className="flex min-h-tap min-w-0 flex-1 items-center gap-3">
                  <input
                    type="checkbox"
                    checked={row.on}
                    disabled={busy}
                    onChange={(e) => set(s.id, { on: e.target.checked })}
                    className="size-5 shrink-0 accent-[var(--accent)]"
                  />
                  <span className="min-w-0">
                    <span className="block text-label tabular-nums">{t("lock.retainSlot", { slot: name })}</span>
                    {size !== null && (
                      <span data-testid={`lock-enough-${s.id}`} className="block text-meta text-ink-secondary">
                        {t(plural(lang, enough, "lock.enough.one", "lock.enough.other"), { count: enough })}
                      </span>
                    )}
                  </span>
                </label>
                <Stepper
                  value={row.count}
                  min={1}
                  max={max}
                  // Toucher le Stepper retient le créneau.
                  onChange={(count) => set(s.id, { count, on: true })}
                  label={t("lock.groups", { slot: name })}
                  disabled={busy}
                />
              </li>
            );
          })}
        </ul>
        {error && (
          <p role="alert" className="mt-3 text-label text-danger">
            {error}
          </p>
        )}
        <div className="mt-5 flex flex-col gap-2">
          <button
            type="button"
            data-testid="lock-submit"
            disabled={reason !== null || busy}
            aria-describedby={reason ? reasonId : undefined}
            onClick={() => void submit()}
            className="min-h-tap rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
          >
            {busy ? t("lock.sending") : t(plural(lang, total, "lock.submit.one", "lock.submit.other"), { count: total })}
          </button>
          {reason && (
            <p id={reasonId} className="text-meta text-ink-secondary">
              {t(reason, { max: GROUPS_MAX })}
            </p>
          )}
          <button
            type="button"
            onClick={close}
            disabled={busy}
            className="min-h-tap rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
          >
            {t("lock.close")}
          </button>
        </div>
      </div>
    </Sheet>
  );
}
