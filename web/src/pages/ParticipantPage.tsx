// Ma candidature `/p/<secret>` (CAP-4, EXPERIENCE.md *Liens et appareils*, *State Patterns*) : en
// tête « Ton lien personnel » avec sa puce, le partage et l'avertissement, puis le statut, le lien
// Discord des acceptés, le résumé public, la candidature en lecture et les actions « Modifier » et
// « Me retirer ». Relecture silencieuse toutes les 10 s, jamais pendant une modification, arrêtée
// sur un lien mort (à la lecture ou sous une commande, qui oublie son secret sur cet appareil), un
// raid annulé ou une candidature retirée. Le secret part en en-tête seulement et n'apparaît que dans la puce, jamais dans
// `document.title`.

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";
import { ApiError } from "../api/client";
import { forgetSecret, saveParticipantLink } from "../api/links";
import { getParticipant, updateCandidature, withdrawCandidature } from "../api/participant";
import { labelOf } from "../api/reference";
import type {
  CandidatureStatus,
  CreatedCandidature,
  ParticipantView,
  ReferenceSet,
  SubmitCandidature,
} from "../api/types";
import { CandidatureForm } from "../components/CandidatureForm";
import { ConfirmSheet } from "../components/ConfirmSheet";
import { CopyChip } from "../components/CopyChip";
import { DeadLink, GONE, isDeadLinkCode } from "../components/DeadLink";
import { FactionDot, Summary, roleLabel } from "../components/EventSummary";
import { t, useLang, type MessageKey } from "../i18n";
import { formatDay, formatSlot } from "../lib/time";
import { REFRESH_MS } from "./EventPage";
import { errorText } from "../lib/form";

/** État de navigation posé par le formulaire de candidature après le succès. */
export interface ParticipantState {
  created: CreatedCandidature;
  /** Faux si `saveParticipantLink` n'a pas pu écrire. */
  stored: boolean;
}

function isState(v: unknown): v is ParticipantState {
  if (typeof v !== "object" || v === null) return false;
  const s = v as Record<string, unknown>;
  return typeof s.stored === "boolean" && typeof s.created === "object" && s.created !== null;
}

const STATUS_KEY: Record<CandidatureStatus, MessageKey> = {
  pending: "participant.status.pending",
  accepted: "participant.status.accepted",
  rejected: "participant.status.rejected",
  withdrawn: "participant.status.withdrawn",
};

/** Badge de statut (DESIGN.md) : `pending` en bleu, `rejected` et `withdrawn` barrés. */
const STATUS_STYLE: Record<CandidatureStatus, string> = {
  pending: "border-pending text-pending",
  accepted: "border-border text-ink-primary",
  rejected: "border-border text-ink-disabled line-through",
  withdrawn: "border-border text-ink-disabled line-through",
};

const isStatus = (v: unknown): v is CandidatureStatus => typeof v === "string" && v in STATUS_KEY;

/** Refus d'état d'une commande : la vue a changé, elle est relue et le geste abandonné. */
const STATE_CODES = new Set([
  "event.locked",
  "event.closed",
  "candidature.withdrawn",
  "candidature.transition_forbidden",
]);

export interface ParticipantPageDeps {
  now: () => number;
}

const defaultDeps: ParticipantPageDeps = { now: () => Date.now() };

interface Props {
  reference: ReferenceSet | null;
  origin?: string;
  deps?: ParticipantPageDeps;
}

export function ParticipantPage({ reference, origin = window.location.origin, deps = defaultDeps }: Props) {
  useLang();
  const { secret = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  // L'état de navigation ne sert qu'au premier rendu après la candidature : il est consommé puis
  // effacé de l'historique, pour qu'un rechargement n'affiche pas un statut périmé.
  const [seed, setSeed] = useState(() => (isState(location.state) ? { secret, state: location.state } : null));
  const navState = seed && seed.secret === secret ? seed.state : null;
  const [view, setView] = useState<ParticipantView | null>(null);
  const [gone, setGone] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [stored, setStored] = useState(navState ? navState.stored : true);
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [withdrawing, setWithdrawing] = useState(false);
  const [withdrawError, setWithdrawError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [announce, setAnnounce] = useState("");
  const editingRef = useRef(false);
  const stopRef = useRef<() => void>(() => {});
  const reloadRef = useRef<() => void>(() => {});
  const savedRef = useRef(false);
  /** Numéro de la dernière requête lancée (lecture ou commande) et de la dernière vue appliquée :
   * une réponse plus ancienne que la vue affichée est ignorée. */
  const issued = useRef(0);
  const applied = useRef(0);
  const lastStatus = useRef<CandidatureStatus | null>(null);
  const restoreFocus = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const statusRef = useRef<HTMLParagraphElement>(null);
  const withdrawButton = useRef<HTMLButtonElement>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const candidatureHeading = useRef<HTMLHeadingElement>(null);
  const url = `${origin}/p/${secret}`;
  const canShare = typeof navigator.share === "function";

  useEffect(() => {
    heading.current?.focus();
  }, []);

  /** Nouvelle vue : statut annoncé s'il change, lien gardé à la première lecture, relecture arrêtée
   * sur un état qui ne bougera plus. `n` = numéro de la requête qui l'a rendue. */
  const apply = useCallback(
    (n: number, v: ParticipantView) => {
      if (n <= applied.current) return;
      applied.current = n;
      setView(v);
      setLoadError(null);
      if (lastStatus.current !== null && lastStatus.current !== v.status) {
        setAnnounce(t("participant.status", { status: t(STATUS_KEY[v.status]) }));
      }
      lastStatus.current = v.status;
      if (v.status === "withdrawn" || v.event.status === "cancelled") stopRef.current();
      if (!savedRef.current) {
        // Ce navigateur retient le lien, même ouvert depuis un autre appareil.
        savedRef.current = true;
        setStored(saveParticipantLink(v.eventId, v.candidatureId, secret));
      }
    },
    [secret],
  );

  useEffect(() => {
    let alive = true;
    let id: ReturnType<typeof setInterval> | undefined;
    const stop = () => clearInterval(id);
    stopRef.current = stop;
    savedRef.current = false;
    // Nouveau secret : toute réponse en vol pour l'ancien est périmée, et l'interface en cours
    // (modification, confirmation) ne doit pas agir sur le nouveau.
    applied.current = issued.current;
    lastStatus.current = null;
    editingRef.current = false;
    setEditing(false);
    setConfirming(false);
    setWithdrawing(false);
    setWithdrawError(null);
    setActionError(null);
    setView(null);
    setGone(false);
    setLoadError(null);
    const load = () => {
      const n = ++issued.current;
      return getParticipant(secret).then(
        (v) => {
          if (alive) apply(n, v);
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
      if (document.visibilityState !== "hidden" && !editingRef.current) load();
    }, REFRESH_MS);
    load();
    return () => {
      alive = false;
      stop();
    };
  }, [secret, apply]);

  useEffect(() => {
    if (!isState(location.state)) return;
    const state = location.state;
    setSeed({ secret, state });
    setStored(state.stored);
    navigate(location.pathname, { replace: true, state: null });
  }, [location.state, location.pathname, secret, navigate]);

  useEffect(() => {
    // Après « Enregistrer » ou « Annuler », le focus revient sur « Modifier », sinon sur le titre de
    // la candidature.
    if (editing || !restoreFocus.current) return;
    restoreFocus.current = false;
    (editButton.current ?? candidatureHeading.current)?.focus();
  }, [editing, view]);

  function edit(on: boolean) {
    editingRef.current = on;
    if (!on) restoreFocus.current = true;
    else setActionError(null);
    setEditing(on);
  }

  /** Lien mort sous une commande : même traitement qu'à la lecture (message unique, relecture
   * arrêtée, secret oublié s'il ne mène plus à rien). Vrai si c'est le cas. */
  function deadUnder(err: unknown): boolean {
    const code = err instanceof ApiError ? err.code : undefined;
    if (!code || !GONE.has(code)) return false;
    stopRef.current();
    editingRef.current = false;
    setEditing(false);
    setConfirming(false);
    setGone(true);
    setView(null);
    if (isDeadLinkCode(code)) forgetSecret(secret);
    return true;
  }

  /** Code de refus d'état, s'il en est un. */
  const stateCode = (err: unknown) =>
    err instanceof ApiError && err.code && STATE_CODES.has(err.code) ? err.code : null;

  async function save(input: SubmitCandidature) {
    const n = ++issued.current;
    try {
      apply(n, await updateCandidature(secret, input));
    } catch (err) {
      if (deadUnder(err)) return;
      const code = stateCode(err);
      // Erreur de champ ou réseau : le formulaire l'affiche et garde la saisie.
      if (!code) throw err;
      // L'état a changé (verrouillé, annulé, retiré) : on quitte la modification et on relit.
      setActionError(errorText(code));
      edit(false);
      reloadRef.current();
      return;
    }
    setActionError(null);
    edit(false);
  }

  async function withdraw() {
    const n = ++issued.current;
    setWithdrawing(true);
    setWithdrawError(null);
    try {
      apply(n, await withdrawCandidature(secret));
      setActionError(null);
      setConfirming(false);
    } catch (err) {
      if (deadUnder(err)) return;
      const code = stateCode(err);
      if (code) {
        setActionError(errorText(code));
        setConfirming(false);
        reloadRef.current();
      } else {
        setWithdrawError(errorText(err instanceof ApiError ? err.code : undefined));
      }
    } finally {
      setWithdrawing(false);
    }
  }

  async function share() {
    try {
      await navigator.share({ url });
    } catch {
      // Partage annulé ou refusé : rien à faire.
    }
  }

  if (gone) return <DeadLink />;

  const status = view?.status ?? (navState && isStatus(navState.created.status) ? navState.created.status : null);
  const statusFocus = status ? statusRef : heading;

  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 ref={heading} tabIndex={-1} className="text-title outline-none">
        {t("participant.title")}
      </h1>
      {!stored && (
        <p role="alert" className="mt-4 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("participant.noStorage")}
        </p>
      )}
      <div className="mt-4">
        <CopyChip value={url} label={t("participant.link")} />
      </div>
      {canShare && (
        <button
          type="button"
          onClick={share}
          className="mt-2 min-h-tap rounded-md border border-border px-4 text-label text-ink-primary"
        >
          {t("participant.share")}
        </button>
      )}
      <p className="mt-2 text-meta text-ink-secondary">{t("participant.warning")}</p>

      {status && (
        <p
          ref={statusRef}
          tabIndex={-1}
          data-testid="status"
          className="mt-6 flex flex-wrap items-center gap-2 text-label outline-none"
        >
          {t("participant.statusLabel")}{" "}
          <span
            data-testid="status-badge"
            data-status={status}
            className={`rounded-full border px-3 py-0.5 text-meta ${STATUS_STYLE[status]}`}
          >
            {t(STATUS_KEY[status])}
          </span>
        </p>
      )}
      {/* Seuls les changements de statut sont annoncés (EXPERIENCE.md *Rafraîchissement*). */}
      <p aria-live="polite" className="sr-only">
        {announce}
      </p>

      {actionError && (
        <p role="alert" data-testid="action-error" className="mt-4 text-label text-danger">
          {actionError}
        </p>
      )}

      {view?.discordUrl && (
        <a
          href={view.discordUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="mt-4 inline-flex min-h-tap items-center rounded-md border border-border px-4 text-label text-ink-primary no-underline"
        >
          {t("participant.discord")}
        </a>
      )}

      {!view ? (
        <div aria-busy="true" className="mt-6">
          <div className="h-6 w-2/3 rounded-sm bg-surface-sunken" />
          <div className="mt-3 h-4 w-1/2 rounded-sm bg-surface-sunken" />
          <div className="mt-2 h-4 w-3/4 rounded-sm bg-surface-sunken" />
          {loadError !== null && (
            <p role="alert" className="mt-4 text-label text-danger">
              {errorText(loadError || undefined)}
            </p>
          )}
        </div>
      ) : (
        <>
          {view.event.status === "cancelled" && (
            <p role="status" className="mt-6 rounded-md border border-danger px-3 py-2 text-label text-danger">
              {t("event.cancelled")}
            </p>
          )}
          <div className="mt-6">
            <Summary event={view.event} reference={reference} now={deps.now} heading="h2" />
          </div>
          {editing ? (
            <CandidatureForm
              event={view.event}
              reference={reference}
              now={deps.now}
              initial={{
                pseudo: view.pseudo,
                server: view.server,
                class: view.class,
                cp: String(view.cp),
                checked: Object.fromEntries(
                  view.availabilities
                    .filter((a) => isUpcoming(view, a.slotId, deps.now))
                    .map((a) => [a.slotId, a.preferred]),
                ),
              }}
              current={{ server: view.server, class: view.class }}
              kept={view.availabilities.filter((a) => isPast(view, a.slotId, deps.now)).length}
              title={t("participant.editTitle")}
              submitLabel={t("participant.save")}
              onSubmit={save}
              onCancel={() => edit(false)}
              cancelLabel={t("participant.cancel")}
            />
          ) : (
            <>
              <Candidature view={view} reference={reference} now={deps.now} headingRef={candidatureHeading} />
              <Actions
                view={view}
                editButton={editButton}
                onEdit={() => edit(true)}
                onWithdraw={() => {
                  setWithdrawError(null);
                  setConfirming(true);
                }}
                withdrawButton={withdrawButton}
              />
            </>
          )}
        </>
      )}

      {confirming && (
        <ConfirmSheet
          message={t("participant.withdrawConfirm")}
          confirmLabel={t("participant.withdraw")}
          cancelLabel={t("participant.cancel")}
          busyLabel={t("event.sending")}
          busy={withdrawing}
          error={withdrawError}
          onConfirm={withdraw}
          onClose={() => setConfirming(false)}
          trigger={withdrawButton}
          fallbackFocus={statusFocus}
        />
      )}
    </main>
  );
}

function slotOf(view: ParticipantView, slotId: string) {
  return view.event.slots.find((s) => s.id === slotId);
}

function isUpcoming(view: ParticipantView, slotId: string, now: () => number): boolean {
  const s = slotOf(view, slotId);
  return !!s && Date.parse(s.startsAt) > now();
}

function isPast(view: ParticipantView, slotId: string, now: () => number): boolean {
  const s = slotOf(view, slotId);
  return !!s && Date.parse(s.startsAt) <= now();
}

interface CandidatureProps {
  view: ParticipantView;
  reference: ReferenceSet | null;
  now: () => number;
  headingRef: RefObject<HTMLHeadingElement | null>;
}

/** La candidature en lecture : pseudo, serveur (faction), classe (rôle), CP, créneaux cochés. */
function Candidature({ view, reference, now, headingRef }: CandidatureProps) {
  const lang = useLang();
  const server = reference?.servers.find((s) => s.id === view.server);
  const gameClass = reference?.classes.find((c) => c.id === view.class);
  const today = new Date(now());
  const row = "flex flex-wrap items-baseline justify-between gap-x-3 border-b border-border py-2";
  return (
    <section data-testid="candidature" className="mt-6" aria-labelledby="candidature-title">
      <h2 ref={headingRef} tabIndex={-1} id="candidature-title" className="text-label outline-none">
        {t("participant.candidature")}
      </h2>
      <dl className="mt-2">
        <div className={row}>
          <dt className="text-meta text-ink-secondary">{t("event.pseudo")}</dt>
          <dd className="font-mono break-all">{view.pseudo}</dd>
        </div>
        <div className={row}>
          <dt className="text-meta text-ink-secondary">{t("event.server")}</dt>
          <dd className="flex items-center gap-1">
            {server ? labelOf(server.label, lang) : view.server}
            {server && reference && (
              <span className="inline-flex items-center gap-1 text-meta text-ink-secondary">
                <FactionDot faction={server.faction} />
                {labelOf(reference.factions[server.faction], lang)}
              </span>
            )}
          </dd>
        </div>
        <div className={row}>
          <dt className="text-meta text-ink-secondary">{t("event.class")}</dt>
          <dd>
            {gameClass ? labelOf(gameClass.label, lang) : view.class}
            {gameClass && (
              <span className="ml-2 text-meta text-ink-secondary">{roleLabel(reference, gameClass.role, lang)}</span>
            )}
          </dd>
        </div>
        <div className={row}>
          <dt className="text-meta text-ink-secondary">{t("event.cp")}</dt>
          <dd className="tabular-nums">{new Intl.NumberFormat(lang).format(view.cp)}</dd>
        </div>
      </dl>
      <h3 className="mt-4 text-meta text-ink-secondary">{t("event.slots")}</h3>
      <ul data-testid="candidature-slots" className="mt-1 flex flex-col gap-1 text-label tabular-nums">
        {view.availabilities.map((a) => {
          const slot = slotOf(view, a.slotId);
          if (!slot) return null;
          const past = Date.parse(slot.startsAt) <= now();
          return (
            <li key={a.slotId} className={`flex items-center gap-2 ${past ? "text-ink-disabled" : ""}`}>
              <span aria-hidden="true">✓</span>
              <span>{formatSlot(slot.startsAt, lang)}</span>
              <span className="text-meta text-ink-secondary">{formatDay(slot.startsAt, lang, today)}</span>
              {a.preferred && (
                <span className="text-accent">
                  <span aria-hidden="true">☆</span>
                  <span className="sr-only">{t("participant.preferred")}</span>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

interface ActionsProps {
  view: ParticipantView;
  onEdit: () => void;
  onWithdraw: () => void;
  withdrawButton: RefObject<HTMLButtonElement | null>;
  editButton: RefObject<HTMLButtonElement | null>;
}

/** « Modifier » si le contenu est modifiable (`open`, non retirée), sinon la raison ; « Me retirer »
 * depuis `pending` ou `accepted` tant que le raid n'est pas annulé. Annulé : aucune action. */
function Actions({ view, onEdit, onWithdraw, withdrawButton, editButton }: ActionsProps) {
  const event = view.event.status;
  if (event === "cancelled") return null;
  const withdrawn = view.status === "withdrawn";
  const canEdit = event === "open" && !withdrawn;
  const canWithdraw = view.status === "pending" || view.status === "accepted";
  const button = "min-h-tap w-full rounded-md border px-4 text-label";
  return (
    <div data-testid="actions" className="mt-6 flex flex-col gap-2">
      {canEdit && (
        <button ref={editButton} type="button" onClick={onEdit} className={`${button} border-border text-ink-primary`}>
          {t("participant.edit")}
        </button>
      )}
      {!canEdit && event === "locked" && !withdrawn && (
        <p data-testid="locked" className="text-meta text-ink-secondary">
          {t(canWithdraw ? "participant.locked" : "participant.lockedNoWithdraw")}
        </p>
      )}
      {withdrawn && (
        <Link
          to={`/e/${view.eventId}`}
          className={`${button} inline-flex items-center justify-center border-border text-ink-primary no-underline`}
        >
          {t("participant.rejoin")}
        </Link>
      )}
      {canWithdraw && (
        <button
          ref={withdrawButton}
          type="button"
          onClick={onWithdraw}
          className={`${button} border-danger text-danger`}
        >
          {t("participant.withdraw")}
        </button>
      )}
    </div>
  );
}
