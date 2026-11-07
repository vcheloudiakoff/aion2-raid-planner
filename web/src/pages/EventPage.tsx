// Page de l'événement `/e/<id>` et candidature directe `/j/<secret>` (CAP-2, CAP-3, EXPERIENCE.md
// *Formulaires*) : un seul écran, le résumé puis le formulaire. Le jeton de formulaire est demandé dès
// le premier rendu (AD-19) ; le résumé se relit toutes les 10 s sans toucher à la saisie. Le secret de
// `/j` part en en-tête seulement, jamais dans `document.title` ni dans le texte de la page. Libellés de
// jeu lus dans la référence, avec repli sur l'id (AD-6).

import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { getEvent, submitCandidature, type EventAccess } from "../api/candidatures";
import { ApiError } from "../api/client";
import { getFormToken } from "../api/events";
import { saveParticipantLink } from "../api/links";
import type { PublicEvent, ReferenceSet, SubmitCandidature } from "../api/types";
import { CandidatureForm } from "../components/CandidatureForm";
import { Summary } from "../components/EventSummary";
import { t, useLang } from "../i18n";
import { readCandidatePrefs, writeCandidatePrefs } from "../lib/storage";
import { errorText } from "../lib/form";

/** Relecture silencieuse (EXPERIENCE.md *Rafraîchissement*), ici et sur Ma candidature. */
export const REFRESH_MS = 10_000;

/** Codes terminaux : un seul message, « ce lien ne mène à rien », et plus aucune relecture. */
export const GONE = new Set(["event.not_found", "auth.not_found", "auth.wrong_right", "auth.missing"]);

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

export function EventPage({ reference, mode, deps = defaultDeps }: Props) {
  useLang();
  const params = useParams();
  const navigate = useNavigate();
  const [prefs] = useState(readCandidatePrefs);
  const key = (mode === "public" ? params.id : params.secret) ?? "";
  const [event, setEvent] = useState<PublicEvent | null>(null);
  const [gone, setGone] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const token = useRef<Promise<string> | null>(null);

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
    const created = await submitCandidature(access, input, formToken, website);
    writeCandidatePrefs({ pseudo: input.pseudo, server: input.server, class: input.class });
    const stored = saveParticipantLink(created.eventId, created.candidatureId, created.participantSecret);
    navigate(created.links.participant, { state: { created, stored } });
  }

  return (
    <main className={main}>
      <Summary event={event} reference={reference} now={deps.now} />
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

/** Lien inconnu, expiré ou d'un autre droit : un seul message, puis le retour à l'accueil. */
export function DeadLink() {
  useLang();
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <p role="alert" className="text-body">
        {t("event.notFound")}
      </p>
      <Link
        to="/"
        className="mt-6 inline-flex min-h-tap items-center rounded-md border border-border px-4 text-label text-ink-primary no-underline"
      >
        {t("event.home")}
      </Link>
    </main>
  );
}
