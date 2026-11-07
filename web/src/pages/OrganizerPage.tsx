// Page organisateur `/o/<secret>` (CAP-5, CAP-6, EXPERIENCE.md *Information Architecture*, *State
// Patterns*, *Liens et appareils*) : titre instance + date, barre d'onglets fixe Candidatures ·
// Disponibilités · Réglages, plus Composition et Salle entre les deux en `locked` (si l'onglet courant
// disparaît, l'affichage revient sur Disponibilités). Candidatures liste toutes les candidatures dans
// l'ordre de création, chaque ligne ouvre sa feuille (accepter, refuser, copier `Pseudo[SRV]`).
// Disponibilités montre la grille joueurs × créneaux (`AvailabilityGrid`), nourrie par la même
// relecture, dont les lignes ouvrent la même feuille, et, en `open`, « Verrouiller » ouvre la feuille
// de verrouillage (`LockSheet`). Composition et Salle listent les groupes retenus (story 3-2 :
// contenu minimal, sans action). Réglages commence par « Ton lien d'organisateur » à copier pour changer
// d'appareil, puis `EventSettings` règle l'événement, ses créneaux et son annulation et propose
// « Dupliquer » ; raid annulé, l'onglet est en lecture seule. Le lien est gardé sur cet appareil à la
// première lecture ; si le navigateur ne peut pas le garder, la page le dit quel que soit l'onglet, en
// renvoyant à Réglages pour le copier. Relecture silencieuse toutes les 10 s, feuille ouverte
// comprise, arrêtée sur un lien mort (qui oublie son secret sur cet appareil) ou un raid annulé ; une
// ligne apparue au rafraîchissement porte un liseré or 3 s. Toutes les commandes passent par
// `command` : une seule à la fois, la relecture l'attend, sa vue s'applique dans l'ordre des
// requêtes. Le secret part en en-tête seulement ; il n'apparaît dans la page que dans la puce de
// Réglages, jamais dans `document.title`.

import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useParams } from "react-router";
import { ApiError } from "../api/client";
import { eventLinks, forgetSecret, saveOrganizerLink } from "../api/links";
import {
  addSlot,
  cancelEvent,
  getOrganizerView,
  lockEvent,
  removeSlot,
  setCandidatureStatus,
  unlockEvent,
  updateEventSettings,
} from "../api/organizer";
import type { Decision, OrganizerEvent, ReferenceSet } from "../api/types";
import { AvailabilityGrid, INITIAL_GRID_CHOICE, type GridChoice } from "../components/AvailabilityGrid";
import { CandidatureRow } from "../components/CandidatureRow";
import { CandidatureSheet } from "../components/CandidatureSheet";
import { CopyChip } from "../components/CopyChip";
import { DeadLink, GONE, isDeadLinkCode } from "../components/DeadLink";
import {
  EventSettings,
  type CommandOutcome,
  type SettingsActions,
  type SettingsCommand,
} from "../components/EventSettings";
import { eventTitle } from "../components/EventSummary";
import { GroupList } from "../components/GroupList";
import { LockSheet } from "../components/LockSheet";
import { t, useLang } from "../i18n";
import { errorText, type Fields } from "../lib/form";
import { REFRESH_MS } from "./EventPage";

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
  "event.not_locked",
  "event.no_upcoming_slot",
]);

/** Commande en vol : une décision de la feuille, un verrouillage, ou un geste de l'onglet Réglages. */
type Pending = Decision | SettingsCommand | "lock";

type Tab = "candidatures" | "availability" | "composition" | "room" | "settings";
const OPEN_TABS: readonly Tab[] = ["candidatures", "availability", "settings"];
const LOCKED_TABS: readonly Tab[] = ["candidatures", "availability", "composition", "room", "settings"];
const TAB_KEY = {
  candidatures: "organizer.tab.candidatures",
  availability: "organizer.tab.availability",
  composition: "organizer.tab.composition",
  room: "organizer.tab.room",
  settings: "organizer.tab.settings",
} as const;

export interface OrganizerPageDeps {
  now: () => number;
}

const defaultDeps: OrganizerPageDeps = { now: () => Date.now() };

interface Props {
  reference: ReferenceSet | null;
  origin?: string;
  deps?: OrganizerPageDeps;
}

export function OrganizerPage({ reference, origin = window.location.origin, deps = defaultDeps }: Props) {
  const lang = useLang();
  const { secret = "" } = useParams();
  const ids = useId();
  const [view, setView] = useState<OrganizerEvent | null>(null);
  const [gone, setGone] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("candidatures");
  /** Choix de l'onglet Disponibilités (vue, filtre, créneau), gardés d'un onglet à l'autre. */
  const [gridChoice, setGridChoice] = useState<GridChoice>(INITIAL_GRID_CHOICE);
  const onGridChoice = useCallback((patch: Partial<GridChoice>) => setGridChoice((c) => ({ ...c, ...patch })), []);
  const [openId, setOpenId] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [sheetError, setSheetError] = useState<string | null>(null);
  const [lockOpen, setLockOpen] = useState(false);
  const [lockError, setLockError] = useState<string | null>(null);
  const lockTrigger = useRef<HTMLElement | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [fresh, setFresh] = useState<ReadonlySet<string>>(() => new Set());
  /** Faux si `saveOrganizerLink` n'a pas pu écrire : ce navigateur ne gardera pas le lien. */
  const [stored, setStored] = useState(true);
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
  const gridHeading = useRef<HTMLHeadingElement>(null);
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
        // Ce navigateur retient le lien, même ouvert depuis un autre appareil.
        savedRef.current = true;
        setStored(saveOrganizerLink(v.event.id, secret));
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
    setLockOpen(false);
    setGridChoice(INITIAL_GRID_CHOICE);
    setPending(null);
    setSheetError(null);
    setActionError(null);
    setView(null);
    setGone(false);
    setLoadError(null);
    setStored(true);
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
            // Lien mort : son entrée locale disparaît de « Sur cet appareil ».
            if (isDeadLinkCode(code)) forgetSecret(secret);
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
        setLockOpen(false);
        setGone(true);
        setView(null);
        if (isDeadLinkCode(api.code)) forgetSecret(secret);
        return { ok: false, handled: true, code: api.code, fields: {} };
      }
      if (api.code && STATE_CODES.has(api.code)) {
        // L'état a changé (annulé, verrouillé, retirée, hors du raid) : on ferme et on relit.
        setActionError(errorText(api.code));
        setOpenId(null);
        setLockOpen(false);
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
    unlock: () => command("unlock", () => unlockEvent(secret)),
  };

  function openLock(from: HTMLButtonElement) {
    lockTrigger.current = from;
    setLockError(null);
    setActionError(null);
    setLockOpen(true);
  }

  /** Rend les erreurs de champ d'un refus, que la feuille applique (créneau commencé). */
  async function lock(slotIds: string[]): Promise<Fields> {
    setLockError(null);
    const outcome = await command("lock", () => lockEvent(secret, slotIds));
    if (outcome.ok) {
      setLockOpen(false);
      return {};
    }
    // Réseau, quota, liste refusée : la feuille reste ouverte avec l'erreur.
    if (!outcome.handled) setLockError(errorText(outcome.code));
    return outcome.fields;
  }

  const status = view?.event.status;
  const TABS = status === "locked" ? LOCKED_TABS : OPEN_TABS;
  // Onglet que la vue dit absent (déverrouillage, relu ou pas) : l'affichage revient sur
  // Disponibilités. Sans vue (chargement), le choix est gardé tel quel.
  const shownTab: Tab = view && !TABS.includes(tab) ? "availability" : tab;
  useEffect(() => {
    // Le repli devient le choix : un reverrouillage ne ramène pas l'onglet disparu.
    if (shownTab !== tab) setTab(shownTab);
  }, [shownTab, tab]);
  // Raid qui n'est plus `open` (verrouillé ailleurs, annulé) : la feuille n'a plus d'objet.
  const lockShown = lockOpen && status === "open";
  useEffect(() => {
    if (lockOpen && view && status !== "open") setLockOpen(false);
  }, [lockOpen, view, status]);

  function onTabKey(e: KeyboardEvent<HTMLButtonElement>) {
    const i = TABS.indexOf(shownTab);
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

  const joinSecret = view ? eventLinks(view.event.id)?.join : undefined;
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
            aria-selected={shownTab === id}
            aria-controls={shownTab === id ? panelId(id) : undefined}
            tabIndex={shownTab === id ? 0 : -1}
            onClick={() => setTab(id)}
            onKeyDown={onTabKey}
            // Largeur selon le libellé ; à 320 px, un libellé passe sur deux lignes plutôt que de déborder.
            className={`min-h-tap min-w-0 flex-auto border-b-2 px-1 text-center text-label [overflow-wrap:anywhere] hyphens-auto ${shownTab === id ? "border-accent text-ink-primary" : "border-transparent text-ink-secondary"}`}
          >
            {t(TAB_KEY[id])}
          </button>
        ))}
      </div>

      {!stored && (
        <p role="alert" className="mt-4 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("organizer.noStorage")}
        </p>
      )}
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

      <div role="tabpanel" id={panelId(shownTab)} aria-labelledby={tabId(shownTab)} className="mt-2">
        {shownTab === "candidatures" ? (
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
        ) : shownTab === "availability" ? (
          <AvailabilityGrid
            view={view}
            reference={reference}
            now={deps.now}
            fresh={fresh}
            onOpen={openSheet}
            choice={gridChoice}
            onChoice={onGridChoice}
            headingRef={gridHeading}
            onLock={openLock}
            busy={pending !== null}
          />
        ) : shownTab === "composition" ? (
          <GroupList view={view} title={t("organizer.tab.composition")} testId="composition" />
        ) : shownTab === "room" ? (
          <GroupList view={view} title={t("organizer.tab.room")} testId="room" />
        ) : (
          <>
            {/* Changer d'appareil : copier ce lien, le coller ailleurs (EXPERIENCE.md *Liens et appareils*). */}
            <section data-testid="my-link" className="mt-4">
              <h2 className="text-label">{t("links.organizer")}</h2>
              <p className="mt-1 text-meta text-ink-secondary">{t("links.organizerWarning")}</p>
              <div className="mt-2">
                <CopyChip value={`${origin}/o/${secret}`} label={t("links.organizer")} />
              </div>
            </section>
            {/* Les deux autres liens, à reposter : le public se déduit de l'id ; le direct n'existe
                qu'en `links.v1` (le serveur n'en garde que l'empreinte), donc sur l'appareil créateur. */}
            <section data-testid="public-link" className="mt-4">
              <h2 className="text-label">{t("links.public")}</h2>
              <p className="mt-1 text-meta text-ink-secondary">{t("links.publicHint")}</p>
              <div className="mt-2">
                <CopyChip value={`${origin}/e/${view.event.id}`} label={t("links.public")} />
              </div>
            </section>
            <section data-testid="join-link" className="mt-4">
              <h2 className="text-label">{t("links.join")}</h2>
              <p className="mt-1 text-meta text-ink-secondary">{t("links.joinHint")}</p>
              {joinSecret ? (
                <div className="mt-2">
                  <CopyChip value={`${origin}/j/${joinSecret}`} label={t("links.join")} />
                </div>
              ) : (
                <p className="mt-2 text-meta text-ink-secondary">{t("organizer.joinMissing")}</p>
              )}
            </section>
            <EventSettings
              view={view}
              reference={reference}
              now={deps.now}
              busy={pending}
              actions={settingsActions}
              fallbackFocus={heading}
            />
          </>
        )}
      </div>

      {lockShown && (
        <LockSheet
          view={view}
          reference={reference}
          now={deps.now}
          busy={pending === "lock"}
          error={lockError}
          onLock={lock}
          onClose={() => setLockOpen(false)}
          trigger={lockTrigger}
          fallbackFocus={gridHeading}
        />
      )}
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
          // Ligne de la grille sortie de la vue (filtre) : le focus revient au titre du panneau.
          fallbackFocus={shownTab === "availability" ? gridHeading : heading}
        />
      )}
    </main>
  );
}
