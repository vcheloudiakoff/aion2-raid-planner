// Feuille d'une candidature, ouverte par sa ligne (EXPERIENCE.md *Component Patterns*) : `Pseudo[SRV]`
// en puce à copier, serveur, classe (rôle), CP, origine, créneaux cochés avec ☆, et les seules
// actions qu'AD-4 permet : « Accepter » depuis `pending` ou `rejected`, « Refuser » depuis `pending`
// ou `accepted`, rien sur `withdrawn` ni sur un raid annulé. « Refuser » passe par une étape de
// confirmation dans la même feuille, jamais une seconde feuille. La feuille lit sa candidature dans
// la vue courante : un rafraîchissement la met à jour.

import { useEffect, useId, useRef, useState, type RefObject } from "react";
import { labelOf } from "../api/reference";
import type { Decision, OrganizerCandidature, PublicEvent, ReferenceSet } from "../api/types";
import { t, useLang } from "../i18n";
import { formatDay, formatSlot } from "../lib/time";
import { CopyChip } from "./CopyChip";
import { DuplicateBadge, StatusBadge } from "./CandidatureRow";
import { FactionDot, roleLabel } from "./EventSummary";
import { Sheet } from "./Sheet";

interface Props {
  candidature: OrganizerCandidature;
  event: PublicEvent;
  reference: ReferenceSet | null;
  now: () => number;
  /** Décision en cours d'envoi, `null` sinon. */
  busy: Decision | null;
  /** Une autre commande de la page est en vol : les décisions attendent qu'elle réponde. */
  otherPending?: boolean;
  /** Erreur du dernier essai, déjà traduite. */
  error: string | null;
  onDecide: (decision: Decision) => void;
  onClose: () => void;
  trigger: RefObject<HTMLElement | null>;
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function CandidatureSheet({
  candidature: c,
  event,
  reference,
  now,
  busy,
  otherPending = false,
  error,
  onDecide,
  onClose,
  trigger,
  fallbackFocus,
}: Props) {
  const lang = useLang();
  const titleId = useId();
  const confirmId = useId();
  const title = useRef<HTMLHeadingElement>(null);
  const statusRef = useRef<HTMLElement>(null);
  const acceptButton = useRef<HTMLButtonElement>(null);
  const rejectButton = useRef<HTMLButtonElement>(null);
  const confirmButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const [confirmStep, setConfirmStep] = useState(false);

  const closed = event.status === "cancelled";
  const canAccept = !closed && (c.status === "pending" || c.status === "rejected");
  const canReject = !closed && (c.status === "pending" || c.status === "accepted");
  // Un rafraîchissement qui rend le refus impossible ferme l'étape de confirmation.
  const confirming = confirmStep && canReject;

  // Focus : « Annuler » à l'entrée de la confirmation (Entrée par réflexe ne détruit rien), puis
  // « Refuser » au retour s'il existe encore, sinon le statut.
  const step = useRef(confirming);
  useEffect(() => {
    if (step.current === confirming) return;
    step.current = confirming;
    if (confirming) cancelButton.current?.focus();
    else (rejectButton.current ?? statusRef.current)?.focus();
  }, [confirming]);

  // Statut changé par une décision : le bouton touché a pu disparaître, le focus va au statut.
  const lastStatus = useRef(c.status);
  useEffect(() => {
    if (lastStatus.current === c.status) return;
    lastStatus.current = c.status;
    setConfirmStep(false);
    if (!document.activeElement || document.activeElement === document.body) statusRef.current?.focus();
  }, [c.status]);

  // Erreur après un envoi : le bouton touché, désactivé le temps de l'envoi, a pu perdre le focus.
  // Il revient dans la feuille, sinon Tab en sortirait.
  useEffect(() => {
    if (!error) return;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    (confirming ? confirmButton.current : (acceptButton.current ?? statusRef.current))?.focus();
  }, [error, confirming]);

  const close = () => {
    if (!busy) onClose();
  };
  const server = reference?.servers.find((s) => s.id === c.server);
  const gameClass = reference?.classes.find((x) => x.id === c.class);
  const today = new Date(now());
  const row = "flex flex-wrap items-baseline justify-between gap-x-3 border-b border-border py-2";
  const button = "min-h-tap rounded-md px-4 text-label disabled:border-border disabled:text-ink-disabled";
  const blocked = busy !== null || otherPending;

  return (
    <Sheet
      labelledBy={confirming ? confirmId : titleId}
      onClose={close}
      trigger={trigger}
      initialFocus={title}
      fallbackFocus={fallbackFocus}
    >
      <h2 ref={title} id={titleId} tabIndex={-1} className="text-title break-all outline-none">
        {c.pseudo}
      </h2>
      {confirming ? (
        <>
          <p id={confirmId} className="mt-3 text-body">
            {t("organizer.rejectConfirm", { name: c.pseudoSrv })}
          </p>
          {error && (
            <p role="alert" className="mt-3 text-label text-danger">
              {error}
            </p>
          )}
          <div className="mt-5 flex flex-col gap-2">
            <button
              ref={confirmButton}
              type="button"
              disabled={blocked}
              onClick={() => onDecide("rejected")}
              className={`${button} border border-danger text-danger`}
            >
              {busy === "rejected" ? t("organizer.sending") : t("organizer.reject")}
            </button>
            <button
              ref={cancelButton}
              type="button"
              disabled={busy !== null}
              onClick={() => setConfirmStep(false)}
              className={`${button} border border-border text-ink-primary`}
            >
              {t("organizer.cancel")}
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="mt-3">
            <CopyChip value={c.pseudoSrv} label={c.pseudoSrv} />
          </div>
          <dl className="mt-3">
            <div className={row}>
              <dt className="text-meta text-ink-secondary">{t("organizer.status")}</dt>
              <dd
                ref={statusRef}
                tabIndex={-1}
                data-testid="sheet-status"
                className="flex flex-wrap gap-1 outline-none"
              >
                {c.duplicate && <DuplicateBadge />}
                <StatusBadge status={c.status} />
              </dd>
            </div>
            <div className={row}>
              <dt className="text-meta text-ink-secondary">{t("event.server")}</dt>
              <dd className="flex items-center gap-1">
                {server ? labelOf(server.label, lang) : c.server}
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
                {gameClass ? labelOf(gameClass.label, lang) : c.class}
                {/* Classe inconnue de la référence : le serveur met son id dans `role`, rien à ajouter. */}
                {c.role !== c.class && (
                  <span className="ml-2 text-meta text-ink-secondary">{roleLabel(reference, c.role, lang)}</span>
                )}
              </dd>
            </div>
            <div className={row}>
              <dt className="text-meta text-ink-secondary">{t("event.cp")}</dt>
              <dd className="tabular-nums">{new Intl.NumberFormat(lang).format(c.cp)}</dd>
            </div>
            <div className={row}>
              <dt className="text-meta text-ink-secondary">{t("organizer.origin")}</dt>
              <dd>{t(c.origin === "direct" ? "organizer.origin.direct" : "organizer.origin.public")}</dd>
            </div>
          </dl>
          <h3 className="mt-3 text-meta text-ink-secondary">{t("organizer.slots")}</h3>
          <ul data-testid="sheet-slots" className="mt-1 flex flex-col gap-1 text-label tabular-nums">
            {c.availabilities.map((a) => {
              const slot = event.slots.find((s) => s.id === a.slotId);
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
                      <span className="sr-only">{t("organizer.preferred")}</span>
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
          {c.status === "withdrawn" && !closed && (
            <p className="mt-3 text-meta text-ink-secondary">{t("organizer.withdrawnNote")}</p>
          )}
          {error && (
            <p role="alert" className="mt-3 text-label text-danger">
              {error}
            </p>
          )}
          <div data-testid="sheet-actions" className="mt-5 flex flex-col gap-2">
            {canAccept && (
              <button
                ref={acceptButton}
                type="button"
                disabled={blocked}
                onClick={() => onDecide("accepted")}
                className={`${button} bg-accent text-on-accent disabled:bg-surface-sunken`}
              >
                {busy === "accepted" ? t("organizer.sending") : t("organizer.accept")}
              </button>
            )}
            {canReject && (
              <button
                ref={rejectButton}
                type="button"
                disabled={blocked}
                onClick={() => setConfirmStep(true)}
                className={`${button} border border-danger text-danger`}
              >
                {t("organizer.reject")}
              </button>
            )}
            <button
              type="button"
              disabled={busy !== null}
              onClick={close}
              className={`${button} border border-border text-ink-primary`}
            >
              {t("organizer.close")}
            </button>
          </div>
        </>
      )}
    </Sheet>
  );
}
