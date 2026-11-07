// Page de l'événement `/e/<id>` et candidature directe `/j/<secret>` (CAP-2, CAP-3, EXPERIENCE.md
// *Formulaires*) : un seul écran, le résumé puis le formulaire. Le jeton de formulaire est demandé dès
// le premier rendu (AD-19) ; le résumé se relit toutes les 10 s sans toucher à la saisie. Le secret de
// `/j` part en en-tête seulement, jamais dans `document.title` ni dans le texte de la page. Libellés de
// jeu lus dans la référence, avec repli sur l'id (AD-6).

import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { getEvent, submitCandidature, type EventAccess } from "../api/candidatures";
import { ApiError } from "../api/client";
import { getFormToken } from "../api/events";
import { saveParticipantLink } from "../api/links";
import { labelOf } from "../api/reference";
import type { Faction, GameClass, PublicEvent, ReferenceSet, Server, SubmitCandidature } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { readCandidatePrefs, writeCandidatePrefs } from "../lib/storage";
import { formatDay, formatSlot, timeZoneLabel } from "../lib/time";
import { errorText } from "./NewEvent";

/** Relecture silencieuse du résumé (EXPERIENCE.md *Rafraîchissement*). */
export const REFRESH_MS = 10_000;
export const PSEUDO_MAX = 16;
export const CP_MAX = 999_999;

/** Codes terminaux : un seul message, « ce lien ne mène à rien », et plus aucune relecture. */
const GONE = new Set(["event.not_found", "auth.not_found", "auth.wrong_right", "auth.missing"]);
const FACTIONS: Faction[] = ["elyos", "asmodian"];

/** Même règle que `domain::validate::pseudo` : 1 à 16 lettres ou chiffres, toute écriture. */
export function isPseudo(s: string): boolean {
  return [...s].length <= PSEUDO_MAX && /^[\p{Alphabetic}\p{N}]+$/u.test(s);
}

function isCp(s: string): boolean {
  return /^\d+$/.test(s.trim()) && Number(s.trim()) <= CP_MAX;
}

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
  if (gone) {
    return (
      <main className={main}>
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
          access={access}
          now={deps.now}
          token={() => token.current ?? Promise.reject(new Error("no token"))}
        />
      )}
    </main>
  );
}

function FactionDot({ faction }: { faction: Faction }) {
  return (
    <span
      aria-hidden="true"
      className={`inline-block size-2 shrink-0 rounded-full ${faction === "elyos" ? "bg-faction-elyos" : "bg-faction-asmodian"}`}
    />
  );
}

function roleLabel(reference: ReferenceSet | null, role: string, lang: string): string {
  const r = reference?.roles.find((x) => x.id === role);
  return r ? labelOf(r.label, lang) : role;
}

interface SummaryProps {
  event: PublicEvent;
  reference: ReferenceSet | null;
  now: () => number;
}

function Summary({ event, reference, now }: SummaryProps) {
  const lang = useLang();
  const instance = reference?.instances.find((i) => i.id === event.instance);
  const region = reference?.regions.find((r) => r.id === event.region);
  const name = instance ? labelOf(instance.label, lang) : event.instance;
  // Titre : le premier créneau à venir, sinon le dernier.
  const first = event.slots.find((s) => Date.parse(s.startsAt) > now()) ?? event.slots[event.slots.length - 1];
  const today = new Date(now());
  const registered = Object.values(event.countsByClass).reduce((a, b) => a + b, 0);
  const roles = [
    t(registered === 1 ? "event.registered.one" : "event.registered.other", { count: registered }),
    ...event.missingRoles.map((m) =>
      t("event.missing", { count: m.missing, role: roleLabel(reference, m.role, lang) }),
    ),
  ].join(" · ");
  const scope = event.factionScope;

  return (
    <section data-testid="summary">
      <h1 className="text-title">{first ? `${name} · ${formatDay(first.startsAt, lang, today)}` : name}</h1>
      <p className="mt-1 flex flex-wrap items-center gap-x-2 text-meta text-ink-secondary">
        <span>{region ? labelOf(region.label, lang) : event.region}</span>
        <span aria-hidden="true">·</span>
        {scope === "all" ? (
          <span>{t("event.scope.all")}</span>
        ) : (
          <span className="inline-flex items-center gap-1">
            <FactionDot faction={scope} />
            {reference ? labelOf(reference.factions[scope], lang) : scope}
          </span>
        )}
      </p>
      {event.note !== "" && <p className="mt-2 text-body break-words">{event.note}</p>}
      <p className="mt-2 text-meta text-ink-secondary">{t("event.timezone", { zone: timeZoneLabel(lang) })}</p>
      <ul data-testid="summary-slots" className="mt-1 flex flex-wrap gap-x-3 text-label tabular-nums">
        {event.slots.map((s) => (
          <li key={s.id} className={Date.parse(s.startsAt) <= now() ? "text-ink-disabled" : ""}>
            {formatSlot(s.startsAt, lang)}{" "}
            <span className="text-meta text-ink-secondary">{formatDay(s.startsAt, lang, today)}</span>
          </li>
        ))}
      </ul>
      <p data-testid="roles" className="mt-2 text-label">
        {roles}
      </p>
      {event.minCp !== undefined && (
        <p className="mt-1 text-meta text-ink-secondary tabular-nums">
          {t("event.minCp", { cp: new Intl.NumberFormat(lang).format(event.minCp) })}
        </p>
      )}
    </section>
  );
}

interface FormProps {
  event: PublicEvent;
  reference: ReferenceSet | null;
  access: EventAccess;
  now: () => number;
  token: () => Promise<string>;
}

type Fields = Record<string, string>;

function CandidatureForm({ event, reference, access, now, token }: FormProps) {
  const lang = useLang();
  const ids = useId();
  const navigate = useNavigate();
  const [prefs] = useState(readCandidatePrefs);
  const [pseudo, setPseudo] = useState(prefs.pseudo ?? "");
  const [serverId, setServerId] = useState(prefs.server ?? "");
  const [classId, setClassId] = useState(prefs.class ?? "");
  const [cp, setCp] = useState("");
  /** Créneaux cochés → préféré. */
  const [checked, setChecked] = useState<Record<string, boolean>>({});
  const [website, setWebsite] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fields, setFields] = useState<Fields>({});
  /** Créneaux envoyés, dans l'ordre : `availabilities.N.slotId` désigne le N-ième. */
  const [submitted, setSubmitted] = useState<string[]>([]);

  const servers: Server[] =
    reference?.servers.filter(
      (s) =>
        s.enabled && s.region === event.region && (event.factionScope === "all" || s.faction === event.factionScope),
    ) ?? [];
  const classes: GameClass[] = reference?.classes.filter((c) => c.enabled && c.id in event.countsByClass) ?? [];
  const server = servers.find((s) => s.id === serverId);
  const gameClass = classes.find((c) => c.id === classId);
  const upcoming = event.slots.filter((s) => Date.parse(s.startsAt) > now());
  const picked = upcoming.filter((s) => s.id in checked);
  const cpValue = cp.trim();
  const belowMin = isCp(cpValue) && event.minCp !== undefined && Number(cpValue) < event.minCp;

  let reason: MessageKey | null = null;
  if (!reference) reason = "event.reason.loading";
  else if (!isPseudo(pseudo)) reason = "event.reason.pseudo";
  else if (!server) reason = "event.reason.server";
  else if (!gameClass) reason = "event.reason.class";
  else if (!isCp(cpValue)) reason = "event.reason.cp";
  else if (picked.length === 0) reason = "event.reason.slots";

  const errorId = (path: string) => `${ids}-err-${path}`;
  const errorLine = (path: string, prefix = false): ReactNode => {
    const code = prefix
      ? Object.entries(fields).find(([k]) => k === path || k.startsWith(`${path}.`))?.[1]
      : fields[path];
    if (!code) return null;
    return (
      <p id={errorId(path)} className="mt-1 text-meta text-danger">
        {errorText(code)}
      </p>
    );
  };
  const invalid = (path: string) => (fields[path] ? { "aria-invalid": true, "aria-describedby": errorId(path) } : {});

  function clearField(path: string) {
    setFields((all) => {
      const keys = Object.keys(all).filter((k) => k === path || k.startsWith(`${path}.`));
      if (keys.length === 0) return all;
      const next = { ...all };
      for (const k of keys) delete next[k];
      return next;
    });
  }

  function toggle(slotId: string) {
    clearField("availabilities");
    setChecked((all) => {
      const next = { ...all };
      if (slotId in next) delete next[slotId];
      else next[slotId] = false;
      return next;
    });
  }

  function togglePreferred(slotId: string) {
    setChecked((all) => (slotId in all ? { ...all, [slotId]: !all[slotId] } : all));
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (reason || sending || !server || !gameClass) return;
    const input: SubmitCandidature = {
      pseudo,
      server: server.id,
      class: gameClass.id,
      cp: Number(cpValue),
      availabilities: picked.map((s) => ({ slotId: s.id, preferred: checked[s.id] === true })),
    };
    setSending(true);
    setError(null);
    setFields({});
    setSubmitted(input.availabilities.map((a) => a.slotId));
    try {
      let formToken: string;
      try {
        formToken = await token();
      } catch {
        formToken = await getFormToken("candidature");
      }
      const created = await submitCandidature(access, input, formToken, website);
      writeCandidatePrefs({ pseudo, server: server.id, class: gameClass.id });
      const stored = saveParticipantLink(created.eventId, created.candidatureId, created.participantSecret);
      navigate(created.links.participant, { state: { created, stored } });
    } catch (err) {
      const api = err instanceof ApiError ? err : new ApiError(0);
      setError(errorText(api.code));
      setFields(api.fields);
      setSending(false);
    }
  }

  const label = "block text-label";
  const control =
    "mt-1 block min-h-tap w-full min-w-0 rounded-md border border-border bg-surface-sunken px-3 text-body text-ink-primary aria-[invalid=true]:border-danger";

  return (
    <form noValidate onSubmit={submit} className="mt-6 flex flex-col gap-5" aria-labelledby={`${ids}-form`}>
      <h2 id={`${ids}-form`} className="text-label">
        {t("event.form")}
      </h2>
      {access.mode === "join" && (
        <p data-testid="direct" className="-mt-3 text-meta text-ink-secondary">
          {t("event.direct")}
        </p>
      )}

      <div>
        <label htmlFor={`${ids}-pseudo`} className={label}>
          {t("event.pseudo")}
        </label>
        <input
          id={`${ids}-pseudo`}
          type="text"
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          value={pseudo}
          onChange={(e) => {
            setPseudo(e.target.value);
            clearField("pseudo");
          }}
          className={control}
          {...invalid("pseudo")}
        />
        {errorLine("pseudo")}
      </div>

      <div>
        <label htmlFor={`${ids}-server`} className={label}>
          {t("event.server")}
        </label>
        <select
          id={`${ids}-server`}
          value={server?.id ?? ""}
          onChange={(e) => {
            setServerId(e.target.value);
            clearField("server");
          }}
          className={control}
          {...invalid("server")}
        >
          <option value="">{t("event.server.choose")}</option>
          {FACTIONS.map((f) => {
            const group = servers.filter((s) => s.faction === f);
            if (group.length === 0 || !reference) return null;
            return (
              <optgroup key={f} label={labelOf(reference.factions[f], lang)}>
                {group.map((s) => (
                  <option key={s.id} value={s.id}>
                    {labelOf(s.label, lang)}
                  </option>
                ))}
              </optgroup>
            );
          })}
        </select>
        {server && reference && (
          <p data-testid="faction" className="mt-1 flex items-center gap-1 text-meta text-ink-secondary">
            <FactionDot faction={server.faction} />
            {t("event.faction", { faction: labelOf(reference.factions[server.faction], lang) })}
          </p>
        )}
        {errorLine("server")}
      </div>

      <div>
        <label htmlFor={`${ids}-class`} className={label}>
          {t("event.class")}
        </label>
        <select
          id={`${ids}-class`}
          value={gameClass?.id ?? ""}
          onChange={(e) => {
            setClassId(e.target.value);
            clearField("class");
          }}
          className={control}
          {...invalid("class")}
        >
          <option value="">{t("event.class.choose")}</option>
          {classes.map((c) => (
            <option key={c.id} value={c.id}>
              {labelOf(c.label, lang)}
            </option>
          ))}
        </select>
        {gameClass && (
          <p data-testid="role" className="mt-1 text-meta text-ink-secondary">
            {t("event.role", { role: roleLabel(reference, gameClass.role, lang) })}
          </p>
        )}
        {errorLine("class")}
      </div>

      <div>
        <label htmlFor={`${ids}-cp`} className={label}>
          {t("event.cp")}
        </label>
        <input
          id={`${ids}-cp`}
          type="number"
          inputMode="numeric"
          min={0}
          max={CP_MAX}
          step={1}
          value={cp}
          onChange={(e) => {
            setCp(e.target.value);
            clearField("cp");
          }}
          className={`${control} tabular-nums`}
          {...invalid("cp")}
        />
        {belowMin && event.minCp !== undefined && (
          <p data-testid="cp-below" className="mt-1 text-meta text-ink-secondary">
            {t("event.cpBelow", { cp: new Intl.NumberFormat(lang).format(event.minCp) })}
          </p>
        )}
        {errorLine("cp")}
      </div>

      <fieldset>
        <legend className={label}>{t("event.slots")}</legend>
        <p className="text-meta text-ink-secondary">{t("event.slots.hint")}</p>
        {upcoming.length === 0 && <p className="mt-2 text-meta text-ink-secondary">{t("event.noUpcoming")}</p>}
        <ul className="mt-2 flex flex-col gap-2">
          {upcoming.map((s) => {
            const on = s.id in checked;
            const preferred = checked[s.id] === true;
            const when = formatSlot(s.startsAt, lang);
            const n = submitted.indexOf(s.id);
            const bad = n >= 0 && !!fields[`availabilities.${n}.slotId`];
            return (
              <li key={s.id} data-testid={`slot-${s.id}`} className="flex items-center gap-2">
                <button
                  type="button"
                  aria-pressed={on}
                  aria-label={t("event.slot.available", { slot: when })}
                  aria-invalid={bad || undefined}
                  aria-describedby={bad ? errorId("availabilities") : undefined}
                  onClick={() => toggle(s.id)}
                  className={`flex min-h-tap min-w-0 flex-1 items-center gap-3 rounded-md border px-3 text-left text-label tabular-nums ${
                    bad
                      ? "border-2 border-danger text-ink-primary"
                      : on
                        ? "border-2 border-available text-ink-primary"
                        : "border-border text-ink-secondary"
                  }`}
                >
                  <span
                    aria-hidden="true"
                    className={`flex size-6 shrink-0 items-center justify-center rounded-sm ${
                      on ? "bg-available text-on-accent" : "bg-surface-sunken"
                    }`}
                  >
                    {on ? "✓" : "–"}
                  </span>
                  <span>{when}</span>
                  <span className="text-meta text-ink-secondary">{formatDay(s.startsAt, lang, new Date(now()))}</span>
                  {bad && (
                    <span aria-hidden="true" className="ml-auto text-danger">
                      !
                    </span>
                  )}
                </button>
                {on && (
                  <button
                    type="button"
                    aria-pressed={preferred}
                    aria-label={t("event.slot.preferred", { slot: when })}
                    onClick={() => togglePreferred(s.id)}
                    className={`flex size-tap shrink-0 items-center justify-center rounded-md border text-title ${
                      preferred ? "border-accent text-accent" : "border-border text-ink-secondary"
                    }`}
                  >
                    {preferred ? "★" : "☆"}
                  </button>
                )}
              </li>
            );
          })}
        </ul>
        {errorLine("availabilities", true)}
      </fieldset>

      {/* Honeypot : invisible et hors du parcours clavier, laissé vide par un humain. */}
      <div aria-hidden="true" className="absolute -left-[9999px] h-px w-px overflow-hidden">
        <label>
          {t("event.honeypot")}
          <input
            type="text"
            name="website"
            tabIndex={-1}
            autoComplete="off"
            value={website}
            onChange={(e) => setWebsite(e.target.value)}
          />
        </label>
      </div>

      <div>
        <button
          type="submit"
          disabled={!!reason || sending}
          aria-describedby={reason ? `${ids}-reason` : undefined}
          className="min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
        >
          {sending ? t("event.sending") : t("event.submit")}
        </button>
        {reason && (
          <p id={`${ids}-reason`} className="mt-2 text-meta text-ink-secondary">
            {t(reason)}
          </p>
        )}
        {error && (
          <p role="alert" className="mt-2 text-label text-danger">
            {error}
          </p>
        )}
      </div>
    </form>
  );
}
