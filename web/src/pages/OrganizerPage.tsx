// Page organisateur `/o/<secret>` (CAP-5, EXPERIENCE.md *Information Architecture*, *State
// Patterns*) : titre instance + date, barre d'onglets fixe Candidatures · Réglages. Candidatures
// liste toutes les candidatures dans l'ordre de création, chaque ligne ouvre sa feuille (accepter,
// refuser, copier `Pseudo[SRV]`). Réglages (`EventSettings`) règle l'événement, ses créneaux et son
// annulation ; raid annulé, l'onglet est en lecture seule. Relecture silencieuse toutes les 10 s,
// feuille ouverte comprise, arrêtée sur un lien mort ou un raid annulé ; une ligne apparue au
// rafraîchissement porte un liseré or 3 s. Toutes les commandes passent par `command` : une seule à
// la fois, la relecture l'attend, sa vue s'applique dans l'ordre des requêtes. Le secret part en
// en-tête seulement et n'apparaît ni dans la page ni dans `document.title`.

import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useParams } from "react-router";
import { ApiError } from "../api/client";
import { saveOrganizerLink } from "../api/links";
import {
  addSlot,
  cancelEvent,
  getOrganizerView,
  removeSlot,
  setCandidatureStatus,
  updateEventSettings,
} from "../api/organizer";
import type { Decision, OrganizerEvent, ReferenceSet } from "../api/types";
import { CandidatureRow } from "../components/CandidatureRow";
import { CandidatureSheet } from "../components/CandidatureSheet";
import {
  EventSettings,
  type CommandOutcome,
  type SettingsActions,
  type SettingsCommand,
} from "../components/EventSettings";
import { eventTitle } from "../components/EventSummary";
import { t, useLang } from "../i18n";
import { errorText } from "../lib/form";
import { DeadLink, GONE, REFRESH_MS } from "./EventPage";

/** Durée du liseré or d'une nouvelle candidature. */
export const FRESH_MS = 3000;

/** Refus d'état d'une commande : la vue a changé, elle est relue et la feuille fermée. */
const STATE_CODES = new Set([
  "event.closed",
  "event.locked",
  "event.field_immutable",
  "candidature.transition_forbidden",
  "candidature.not_found",
  "slot.not_found",
]);

/** Commande en vol : une décision de la feuille, ou un geste de l'onglet Réglages. */
type Pending = Decision | SettingsCommand;

const TABS = ["candidatures", "settings"] as const;
type Tab = (typeof TABS)[number];
const TAB_KEY = { candidatures: "organizer.tab.candidatures", settings: "organizer.tab.settings" } as const;

export interface OrganizerPageDeps {
  now: () => number;
}

const defaultDeps: OrganizerPageDeps = { now: () => Date.now() };

interface Props {
  reference: ReferenceSet | null;
  deps?: OrganizerPageDeps;
}

export function OrganizerPage({ reference, deps = defaultDeps }: Props) {
  const lang = useLang();
  const { secret = "" } = useParams();
  const ids = useId();
  const [view, setView] = useState<OrganizerEvent | null>(null);
  const [gone, setGone] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("candidatures");
  const [openId, setOpenId] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [sheetError, setSheetError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [fresh, setFresh] = useState<ReadonlySet<string>>(() => new Set());
  /** Numéro de la dernière requête lancée (lecture ou commande) et de la dernière vue appliquée :
   * une réponse plus ancienne que la vue affichée est ignorée. */
  const issued = useRef(0);
  const applied = useRef(0);
  /** Ids de la dernière vue appliquée ; `null` avant la première. */
  const known = useRef<ReadonlySet<string> | null>(null);
  const freshTimers = useRef(new Set<ReturnType<typeof setTimeout>>());
  const stopRef = useRef<() => void>(() => {});
  const reloadRef = useRef<() => void>(() => {});
  const savedRef = useRef(false);
  /** Commande en vol : la relecture périodique attend sa réponse. */
  const busyRef = useRef(false);
  /** Requête de la relecture qui suit un refus d'état : son message reste jusqu'à une vue plus
   * récente. */
  const errorAt = useRef(0);
  const trigger = useRef<HTMLElement | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const tabRefs = useRef<Partial<Record<Tab, HTMLButtonElement | null>>>({});

  const clearFresh = useCallback(() => {
    for (const id of freshTimers.current) clearTimeout(id);
    freshTimers.current.clear();
    setFresh(new Set());
  }, []);

  /** Liseré or sur `added` pendant `FRESH_MS`. */
  const mark = useCallback((added: string[]) => {
    setFresh((prev) => new Set([...prev, ...added]));
    const timer = setTimeout(() => {
      freshTimers.current.delete(timer);
      setFresh((prev) => {
        const next = new Set(prev);
        for (const id of added) next.delete(id);
        return next;
      });
    }, FRESH_MS);
    freshTimers.current.add(timer);
  }, []);

  /** Nouvelle vue, rendue par la requête `n`. Le liseré ne marque qu'une candidature absente de la
   * vue précédente, sur une relecture : jamais au premier chargement ni sur une commande. Le lien est
   * gardé à la première lecture ; la relecture s'arrête sur un raid annulé. */
  const apply = useCallback(
    (n: number, v: OrganizerEvent, source: "read" | "command") => {
      if (n <= applied.current) return;
      applied.current = n;
      const previous = known.current;
      known.current = new Set(v.candidatures.map((c) => c.id));
      if (source === "read" && previous) {
        const added = v.candidatures.filter((c) => !previous.has(c.id)).map((c) => c.id);
        if (added.length > 0) mark(added);
      }
      setView(v);
      setLoadError(null);
      // Le message d'un refus d'état survit à la relecture qu'il a lancée, puis s'efface.
      if (n > errorAt.current) setActionError(null);
      if (v.event.status === "cancelled") stopRef.current();
      if (!savedRef.current) {
        savedRef.current = true;
        saveOrganizerLink(v.event.id, secret);
      }
    },
    [secret, mark],
  );

  useEffect(() => {
    let alive = true;
    let id: ReturnType<typeof setInterval> | undefined;
    const stop = () => clearInterval(id);
    stopRef.current = stop;
    savedRef.current = false;
    // Nouveau secret : toute réponse en vol pour l'ancien est périmée, et la feuille ouverte ne doit
    // pas agir sur le nouveau.
    applied.current = issued.current;
    known.current = null;
    busyRef.current = false;
    clearFresh();
    setOpenId(null);
    setPending(null);
    setSheetError(null);
    setActionError(null);
    setView(null);
    setGone(false);
    setLoadError(null);
    const load = () => {
      const n = ++issued.current;
      return getOrganizerView(secret).then(
        (v) => {
          if (alive) apply(n, v, "read");
        },
        (err: unknown) => {
          if (!alive || n <= applied.current) return;
          const code = err instanceof ApiError ? err.code : undefined;
          if (code && GONE.has(code)) {
            stop();
            setGone(true);
            setView(null);
          } else {
            // Une relecture qui échoue garde les dernières données : seul un premier échec s'affiche.
            setLoadError(code ?? "");
          }
        },
      );
    };
    reloadRef.current = () => void load();
    id = setInterval(() => {
      if (document.visibilityState !== "hidden" && !busyRef.current) load();
    }, REFRESH_MS);
    load();
    return () => {
      alive = false;
      stop();
      clearFresh();
    };
  }, [secret, apply, clearFresh]);

  const open = openId && view ? (view.candidatures.find((c) => c.id === openId) ?? null) : null;
  useEffect(() => {
    // Candidature disparue de la vue : la feuille n'a plus rien à montrer.
    if (openId && view && !open) setOpenId(null);
  }, [openId, view, open]);

  function openSheet(id: string, from: HTMLButtonElement) {
    trigger.current = from;
    setSheetError(null);
    setActionError(null);
    setOpenId(id);
  }

  /** Lance une commande, une à la fois : la relecture périodique attend sa réponse, et la vue rendue
   * s'applique si aucune plus récente ne l'a précédée. Un lien mort ou un refus d'état est traité ici
   * (message dans la page, feuille fermée, vue relue) ; les autres refus reviennent à l'appelant, qui
   * les montre près du geste. */
  async function command(kind: Pending, send: () => Promise<OrganizerEvent>): Promise<CommandOutcome> {
    if (busyRef.current) return { ok: false, handled: true, fields: {} };
    const n = ++issued.current;
    busyRef.current = true;
    setPending(kind);
    try {
      const v = await send();
      apply(n, v, "command");
      return { ok: true, view: v };
    } catch (err) {
      const api = err instanceof ApiError ? err : new ApiError(0);
      if (api.code && GONE.has(api.code)) {
        stopRef.current();
        setOpenId(null);
        setGone(true);
        setView(null);
        return { ok: false, handled: true, code: api.code, fields: {} };
      }
      if (api.code && STATE_CODES.has(api.code)) {
        // L'état a changé (annulé, verrouillé, retirée, hors du raid) : on ferme et on relit.
        setActionError(errorText(api.code));
        setOpenId(null);
        reloadRef.current();
        errorAt.current = issued.current;
        return { ok: false, handled: true, code: api.code, fields: api.fields };
      }
      // Réseau, quota, champ refusé : à l'appelant.
      return { ok: false, handled: false, code: api.code, fields: api.fields };
    } finally {
      busyRef.current = false;
      setPending(null);
    }
  }

  async function decide(decision: Decision) {
    if (!openId) return;
    setSheetError(null);
    const outcome = await command(decision, () => setCandidatureStatus(secret, openId, decision));
    // Réseau, quota : la feuille reste ouverte avec l'erreur.
    if (!outcome.ok && !outcome.handled) setSheetError(errorText(outcome.code));
  }

  const settingsActions: SettingsActions = {
    save: (body) => command("settings", () => updateEventSettings(secret, body)),
    addSlot: (slot) => command("addSlot", () => addSlot(secret, slot)),
    removeSlot: (slotId) => command("removeSlot", () => removeSlot(secret, slotId)),
    cancel: () => command("cancel", () => cancelEvent(secret)),
  };

  function onTabKey(e: KeyboardEvent<HTMLButtonElement>) {
    const i = TABS.indexOf(tab);
    const next =
      e.key === "ArrowRight"
        ? TABS[(i + 1) % TABS.length]
        : e.key === "ArrowLeft"
          ? TABS[(i + TABS.length - 1) % TABS.length]
          : e.key === "Home"
            ? TABS[0]
            : e.key === "End"
              ? TABS[TABS.length - 1]
              : null;
    if (!next) return;
    e.preventDefault();
    setTab(next);
    tabRefs.current[next]?.focus();
  }

  if (gone) return <DeadLink />;

  const main = "mx-auto max-w-content px-3 py-6 min-[360px]:px-4";
  if (!view) {
    return (
      <main className={main} aria-busy="true">
        <div className="h-6 w-2/3 rounded-sm bg-surface-sunken" />
        <div className="mt-4 h-tap rounded-sm bg-surface-sunken" />
        <div className="mt-3 h-12 rounded-sm bg-surface-sunken" />
        <div className="mt-2 h-12 rounded-sm bg-surface-sunken" />
        {loadError !== null && (
          <p role="alert" className="mt-4 text-label text-danger">
            {errorText(loadError || undefined)}
          </p>
        )}
      </main>
    );
  }

  const tabId = (id: Tab) => `${ids}-tab-${id}`;
  const panelId = (id: Tab) => `${ids}-panel-${id}`;

  return (
    <main className={main}>
      <h1 ref={heading} tabIndex={-1} className="text-title outline-none">
        {eventTitle(view.event, reference, lang, deps.now)}
      </h1>
      <div
        role="tablist"
        aria-label={t("organizer.tabs")}
        className="sticky top-0 z-10 -mx-3 mt-3 flex border-b border-border bg-surface-base px-3 min-[360px]:-mx-4 min-[360px]:px-4"
      >
        {TABS.map((id) => (
          <button
            key={id}
            ref={(el) => {
              tabRefs.current[id] = el;
            }}
            type="button"
            role="tab"
            id={tabId(id)}
            aria-selected={tab === id}
            aria-controls={tab === id ? panelId(id) : undefined}
            tabIndex={tab === id ? 0 : -1}
            onClick={() => setTab(id)}
            onKeyDown={onTabKey}
            className={`min-h-tap flex-1 border-b-2 px-3 text-label ${tab === id ? "border-accent text-ink-primary" : "border-transparent text-ink-secondary"}`}
          >
            {t(TAB_KEY[id])}
          </button>
        ))}
      </div>

      {view.event.status === "cancelled" && (
        <p role="status" className="mt-4 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("event.cancelled")}
        </p>
      )}
      {actionError && (
        <p role="alert" data-testid="action-error" className="mt-4 text-label text-danger">
          {actionError}
        </p>
      )}

      <div role="tabpanel" id={panelId(tab)} aria-labelledby={tabId(tab)} className="mt-2">
        {tab === "candidatures" ? (
          view.candidatures.length === 0 ? (
            <p className="mt-4 text-body text-ink-secondary">{t("organizer.empty")}</p>
          ) : (
            <ul data-testid="candidatures">
              {view.candidatures.map((c) => (
                <CandidatureRow
                  key={c.id}
                  candidature={c}
                  reference={reference}
                  fresh={fresh.has(c.id)}
                  onOpen={openSheet}
                />
              ))}
            </ul>
          )
        ) : (
          <EventSettings
            view={view}
            reference={reference}
            now={deps.now}
            busy={pending}
            actions={settingsActions}
            fallbackFocus={heading}
          />
        )}
      </div>

      {open && (
        <CandidatureSheet
          candidature={open}
          event={view.event}
          reference={reference}
          now={deps.now}
          busy={pending === "accepted" || pending === "rejected" ? pending : null}
          otherPending={pending !== null && pending !== "accepted" && pending !== "rejected"}
          error={sheetError}
          onDecide={decide}
          onClose={() => setOpenId(null)}
          trigger={trigger}
          fallbackFocus={heading}
        />
      )}
    </main>
  );
}
