// Page de l'événement `/e/<id>` et candidature directe `/j/<secret>` (CAP-2, CAP-3, EXPERIENCE.md
// *Formulaires*) : un seul écran, le résumé puis le formulaire. Le jeton de formulaire est demandé dès
// le premier rendu (AD-19) ; le résumé se relit toutes les 10 s sans toucher à la saisie. Le secret de
// `/j` part en en-tête seulement, jamais dans `document.title` ni dans le texte de la page. Lien mort, à
// la lecture comme à l'envoi : `/e` oublie son événement sur `event.not_found`, `/j` oublie son secret.
// Libellés de jeu lus dans la référence, avec repli sur l'id (AD-6).

import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { getEvent, submitCandidature, type EventAccess } from "../api/candidatures";
import { ApiError } from "../api/client";
import { getFormToken } from "../api/events";
import { forgetEvent, forgetSecret, saveParticipantLink } from "../api/links";
import type { CreatedCandidature, JoinEvent, PublicEvent, ReferenceSet, SubmitCandidature } from "../api/types";
import { Roster } from "../components/AcceptedList";
import { CandidatureForm } from "../components/CandidatureForm";
import { DeadLink, GONE, isDeadLinkCode } from "../components/DeadLink";
import { Summary } from "../components/EventSummary";
import { t, useLang } from "../i18n";
import { readCandidatePrefs, writeCandidatePrefs } from "../lib/storage";
import { errorText } from "../lib/form";

/** Relecture silencieuse (EXPERIENCE.md *Rafraîchissement*), ici et sur Ma candidature. */
export const REFRESH_MS = 10_000;

/** Sous-ensemble à tester sans horloge réelle. */
export interface EventPageDeps {
  now: () => number;
}

interface Props {
  reference: ReferenceSet | null;
  mode: "public" | "join";
  deps?: EventPageDeps;
}

const defaultDeps: EventPageDeps = { now: () => Date.now() };

/** Lien mort : son entrée locale disparaît de « Sur cet appareil », le secret de `/j` ou
 * l'événement de `/e`. */
function forgetDead(access: EventAccess, code: string) {
  if (access.mode === "join" && isDeadLinkCode(code)) forgetSecret(access.secret);
  if (access.mode === "public" && code === "event.not_found") forgetEvent(access.eventId);
}

export function EventPage({ reference, mode, deps = defaultDeps }: Props) {
  useLang();
  const params = useParams();
  const navigate = useNavigate();
  const [prefs] = useState(readCandidatePrefs);
  const key = (mode === "public" ? params.id : params.secret) ?? "";
  const [event, setEvent] = useState<PublicEvent | JoinEvent | null>(null);
  const [gone, setGone] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const token = useRef<Promise<string> | null>(null);
  const stopRef = useRef<() => void>(() => {});

  useEffect(() => {
    // Demandé au premier rendu, sans bloquer l'écran ; un échec est rattrapé à l'envoi.
    const p = getFormToken("candidature");
    p.catch(() => {});
    token.current = p;
  }, []);

  useEffect(() => {
    const access: EventAccess = mode === "public" ? { mode, eventId: key } : { mode, secret: key };
    let alive = true;
    let id: ReturnType<typeof setInterval> | undefined;
    const stop = () => clearInterval(id);
    stopRef.current = stop;
    setEvent(null);
    setGone(false);
    setLoadError(null);
    const load = () =>
      getEvent(access).then(
        (e) => {
          if (!alive) return;
          setEvent(e);
          setLoadError(null);
          // Un raid annulé ne changera plus : la page reste en lecture seule.
          if (e.status === "cancelled") stop();
        },
        (err: unknown) => {
          if (!alive) return;
          const code = err instanceof ApiError ? err.code : undefined;
          if (code && GONE.has(code)) {
            stop();
            setGone(true);
            setEvent(null);
            forgetDead(access, code);
          } else {
            // Une relecture qui échoue garde les dernières données : seul un premier échec s'affiche.
            setLoadError(code ?? "");
          }
        },
      );
    id = setInterval(() => {
      if (document.visibilityState !== "hidden") load();
    }, REFRESH_MS);
    load();
    return () => {
      alive = false;
      stop();
    };
  }, [mode, key]);

  const main = "mx-auto max-w-content px-3 py-6 min-[360px]:px-4";
  if (gone) return <DeadLink />;
  if (!event) {
    return (
      <main className={main} aria-busy="true">
        <div className="h-6 w-2/3 rounded-sm bg-surface-sunken" />
        <div className="mt-3 h-4 w-1/2 rounded-sm bg-surface-sunken" />
        <div className="mt-2 h-4 w-3/4 rounded-sm bg-surface-sunken" />
        {loadError !== null && (
          <p role="alert" className="mt-4 text-label text-danger">
            {errorText(loadError || undefined)}
          </p>
        )}
      </main>
    );
  }
  const access: EventAccess = mode === "public" ? { mode, eventId: event.id } : { mode, secret: key };

  async function send(input: SubmitCandidature, website: string) {
    let formToken: string;
    try {
      formToken = await (token.current ?? Promise.reject(new Error("no token")));
    } catch {
      formToken = await getFormToken("candidature");
    }
    let created: CreatedCandidature;
    try {
      created = await submitCandidature(access, input, formToken, website);
    } catch (err) {
      const code = err instanceof ApiError ? err.code : undefined;
      // Lien mort à l'envoi : même traitement qu'à la lecture, le formulaire laisse place au message.
      if (!code || !GONE.has(code)) throw err;
      stopRef.current();
      setGone(true);
      setEvent(null);
      forgetDead(access, code);
      return;
    }
    writeCandidatePrefs({ pseudo: input.pseudo, server: input.server, class: input.class });
    const stored = saveParticipantLink(created.eventId, created.candidatureId, created.participantSecret);
    navigate(created.links.participant, { state: { created, stored } });
  }

  return (
    <main className={main}>
      <Summary event={event} reference={reference} now={deps.now} />
      {access.mode === "join" && "acceptedRoster" in event && event.acceptedRoster.length > 0 && (
        <Roster entries={event.acceptedRoster} self={undefined} slots={event.slots} reference={reference} />
      )}
      {event.status === "cancelled" ? (
        <p role="status" className="mt-6 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("event.cancelled")}
        </p>
      ) : (
        <CandidatureForm
          event={event}
          reference={reference}
          now={deps.now}
          initial={prefs}
          title={t("event.form")}
          intro={
            access.mode === "join" && (
              <p data-testid="direct" className="-mt-3 text-meta text-ink-secondary">
                {t("event.direct")}
              </p>
            )
          }
          submitLabel={t("event.submit")}
          honeypot
          onSubmit={send}
        />
      )}
    </main>
  );
}
