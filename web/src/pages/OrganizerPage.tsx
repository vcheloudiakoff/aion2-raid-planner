// Page organisateur `/o/<secret>` (CAP-5, CAP-6, EXPERIENCE.md *Information Architecture*, *State
// Patterns*, *Liens et appareils*) : titre instance + date, barre d'onglets fixe Candidatures ·
// Disponibilités · Réglages, plus Composition et Salle entre les deux en `locked` (si l'onglet courant
// disparaît, l'affichage revient sur Disponibilités). Candidatures liste toutes les candidatures dans
// l'ordre de création, chaque ligne ouvre sa feuille (accepter, refuser, copier `Pseudo[SRV]`).
// Disponibilités montre la grille joueurs × créneaux (`AvailabilityGrid`), nourrie par la même
// relecture, dont les lignes ouvrent la même feuille, et, en `open`, « Verrouiller » ouvre la feuille
// de verrouillage (`LockSheet`). Composition (`Composition`, chargée à la demande dans un chunk à
// part, avec un squelette en attendant) place titulaires et banc des groupes retenus : le brouillon
// vit ici, pour survivre à un changement d'onglet, avec sa vue de base pour le bandeau « N
// candidatures ont changé » ; une relecture ne l'écrase jamais, « Enregistrer » envoie le placement
// complet, un refus le garde. Salle (`Room`, chargée à la demande dans son propre chunk) montre une
// carte par groupe retenu (nom à copier, recette, slots et banc) et « Copier le message Discord »,
// construit côté client avec l'origine du navigateur, sans secret, et signale un brouillon de
// composition non enregistré ; « Régénérer le nom » passe par `command`, et un groupe qui n'est plus
// retenu y est un refus d'état. Réglages
// commence par « Ton lien d'organisateur » à copier pour changer d'appareil, puis `EventSettings`
// règle l'événement, ses créneaux et son annulation et propose
// « Dupliquer » ; raid annulé, l'onglet est en lecture seule. Le lien est gardé sur cet appareil à la
// première lecture ; si le navigateur ne peut pas le garder, la page le dit quel que soit l'onglet, en
// renvoyant à Réglages pour le copier. Relecture silencieuse toutes les 10 s, feuille ouverte
// comprise, arrêtée sur un lien mort (qui oublie son secret sur cet appareil) ou un raid annulé ; une
// ligne apparue au rafraîchissement porte un liseré or 3 s. Toutes les commandes passent par
// `command` : une seule à la fois, la relecture l'attend, sa vue s'applique dans l'ordre des
// requêtes. Le secret part en en-tête seulement ; il n'apparaît dans la page que dans la puce de
// Réglages, jamais dans `document.title`.

import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { useParams } from "react-router";
import { ApiError } from "../api/client";
import { eventLinks, forgetSecret, saveOrganizerLink } from "../api/links";
import {
  addSlot,
  applyPlacement,
  cancelEvent,
  getOrganizerView,
  lockEvent,
  regenerateRoomName,
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
import { LockSheet } from "../components/LockSheet";
import { t, useLang } from "../i18n";
import { errorText, type Fields } from "../lib/form";
import { bodyOf, changedCount, type Draft, type Placement, type Seat } from "../lib/placement";
import type { SeatError } from "../components/Composition";
import type { RegenerateOutcome } from "../components/Room";
import { REFRESH_MS } from "./EventPage";

/** Onglet Composition, chargé à la demande : rien de ce module n'est importé statiquement ici. */
const Composition = lazy(() => import("../components/Composition"));
/** Onglet Salle, chargé à la demande de la même façon. */
const Room = lazy(() => import("../components/Room"));

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

/** Refus d'état propre à « Régénérer le nom » : le groupe n'est plus retenu (ou plus là). Ailleurs,
 * `placement.unknown_group` est un refus structurel du brouillon, qui le garde. */
const ROOM_STATE_CODES: ReadonlySet<string> = new Set(["placement.unknown_group"]);

/** Commande en vol : une décision de la feuille, un verrouillage, un placement, un nouveau nom de
 * salle, ou un geste de l'onglet Réglages. */
type Pending = Decision | SettingsCommand | "lock" | "placement" | "roomName";

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
  /** Brouillon de composition : créé au premier geste, gardé d'un onglet à l'autre, jamais écrasé
   * par une relecture ; effacé au changement de secret. */
  const [draft, setDraft] = useState<Draft | null>(null);
  const [placementError, setPlacementError] = useState<string | null>(null);
  /** Places désignées par les champs du dernier refus structurel. */
  const [seatErrors, setSeatErrors] = useState<SeatError[]>([]);
  /** Le brouillon a été abandonné parce que le raid a changé sous lui (déverrouillage, groupes). */
  const [draftDropped, setDraftDropped] = useState(false);
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
    setDraft(null);
    setPlacementError(null);
    setSeatErrors([]);
    setDraftDropped(false);
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
   * les montre près du geste. `stateCodes` : codes que ce geste traite en plus comme refus d'état. */
  async function command(
    kind: Pending,
    send: () => Promise<OrganizerEvent>,
    stateCodes?: ReadonlySet<string>,
  ): Promise<CommandOutcome> {
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
      if (api.code && (STATE_CODES.has(api.code) || stateCodes?.has(api.code))) {
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

  /** « Régénérer le nom » : la confirmation se ferme sur une réussite ou un refus d'état (dit dans la
   * page, vue relue ; un groupe qui n'est plus retenu a son propre texte) ; elle reste ouverte avec
   * l'erreur (réseau, quota), ou sans rien dire si une autre commande est en vol. */
  async function regenerateRoom(groupId: string): Promise<RegenerateOutcome> {
    if (busyRef.current) return { close: false, error: null };
    const outcome = await command("roomName", () => regenerateRoomName(secret, groupId), ROOM_STATE_CODES);
    if (outcome.ok) return { close: true };
    if (outcome.handled) {
      if (outcome.code === "placement.unknown_group") setActionError(t("room.unknownGroup"));
      return { close: true };
    }
    return { close: false, error: errorText(outcome.code) };
  }

  /** Premier geste : le brouillon commence sur la vue affichée, sa base pour le bandeau. Un geste
   * efface les places d'un refus précédent : leurs rangs ne valent plus. */
  const editPlacement = useCallback(
    (placement: Placement) => {
      setDraft((d) => (d ? { ...d, placement } : view ? { base: view, placement } : null));
      setDraftDropped(false);
      setSeatErrors([]);
      setPlacementError(null);
    },
    [view],
  );

  // Le raid a changé sous le brouillon (déverrouillé, annulé, ou autres groupes retenus) : ses
  // groupes ne valent plus, il est abandonné, et la page le dit.
  useEffect(() => {
    if (!draft || !view) return;
    const retained = (v: OrganizerEvent) =>
      v.groups
        .filter((g) => g.retained)
        .map((g) => g.id)
        .sort()
        .join(",");
    if (view.event.status !== "locked" || retained(view) !== retained(draft.base)) {
      setDraft(null);
      setPlacementError(null);
      setSeatErrors([]);
      setDraftDropped(true);
    }
  }, [draft, view]);

  /** « Enregistrer » : le placement complet du brouillon. Un refus garde le brouillon ; un refus
   * structurel désigne ses places par l'ordre du corps envoyé (`placements.N.<champ>`). */
  async function savePlacement() {
    if (!draft) return;
    setPlacementError(null);
    setSeatErrors([]);
    const body = bodyOf(draft.placement);
    const outcome = await command("placement", () => applyPlacement(secret, body));
    if (outcome.ok) {
      setDraft(null);
      return;
    }
    // Structure refusée (une candidature n'est plus acceptée…), réseau, quota : près du bouton.
    if (!outcome.handled) setPlacementError(errorText(outcome.code));
    const seats: SeatError[] = [];
    for (const [path, code] of Object.entries(outcome.fields ?? {})) {
      const entry = body[Number(/^placements\.(\d+)\./.exec(path)?.[1] ?? NaN)];
      if (!entry) continue;
      const seat: Seat =
        "benchIndex" in entry
          ? { groupId: entry.groupId, bench: entry.benchIndex - 1 }
          : { groupId: entry.groupId, party: entry.party, index: entry.index };
      seats.push({ seat, message: errorText(code) });
    }
    setSeatErrors(seats);
  }

  function discardDraft() {
    setDraft(null);
    setPlacementError(null);
    setSeatErrors([]);
  }

  /** « Voir » : le brouillon est abandonné et la vue relue. */
  function seeChanges() {
    discardDraft();
    if (!busyRef.current) reloadRef.current();
  }

  const changed = useMemo(() => (draft && view ? changedCount(draft.base, view) : 0), [draft, view]);

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
      {draftDropped && (
        <p role="status" data-testid="draft-dropped" className="mt-4 text-label text-ink-secondary">
          {t("composition.dropped")}
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
          <Suspense
            fallback={
              <div data-testid="composition-loading" aria-busy="true" className="mt-4">
                <div className="h-tap w-1/2 rounded-sm bg-surface-sunken" />
                <div className="mt-3 h-[52px] rounded-md bg-surface-sunken" />
                <div className="mt-1 h-[52px] rounded-md bg-surface-sunken" />
              </div>
            }
          >
            <Composition
              view={view}
              reference={reference}
              draft={draft?.placement ?? null}
              onEdit={editPlacement}
              onSave={() => void savePlacement()}
              onDiscard={discardDraft}
              onSee={seeChanges}
              changed={changed}
              saving={pending === "placement"}
              busy={pending !== null}
              error={placementError}
              seatErrors={seatErrors}
              fallbackFocus={heading}
            />
          </Suspense>
        ) : shownTab === "room" ? (
          <Suspense
            fallback={
              <div data-testid="room-loading" aria-busy="true" className="mt-4">
                <div className="h-tap rounded-md bg-surface-sunken" />
                <div className="mt-4 h-40 rounded-md bg-surface-sunken" />
              </div>
            }
          >
            <Room
              view={view}
              reference={reference}
              origin={origin}
              onRegenerate={regenerateRoom}
              regenerating={pending === "roomName"}
              busy={pending !== null}
              draft={draft !== null}
              fallbackFocus={heading}
            />
          </Suspense>
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
