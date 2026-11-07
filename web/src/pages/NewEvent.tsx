// Écran `/new` (CAP-1, EXPERIENCE.md *Formulaires*) : un seul écran à 360 px, puis l'étape « Liens
// créés ». Le jeton de formulaire est demandé dès le premier rendu, en parallèle (AD-19). Les heures
// sont saisies en heure locale et envoyées en UTC (AD-11). Libellés de jeu lus dans la référence.

import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";
import { ApiError } from "../api/client";
import { createEvent, getFormToken } from "../api/events";
import { saveOrganizerLinks } from "../api/links";
import { labelOf } from "../api/reference";
import type { CreateEvent, CreatedEvent, FactionScope, Instance, ReferenceSet } from "../api/types";
import { Stepper } from "../components/Stepper";
import { hasKey, t, useLang, type MessageKey } from "../i18n";
import { localInputToUtc, timeZoneLabel } from "../lib/time";
import { LinksCreated } from "./LinksCreated";

export const NOTE_MAX = 280;
export const SLOTS_MAX = 14;

interface SlotDraft {
  key: number;
  /** Valeur de `datetime-local`, heure locale. */
  local: string;
  /** Vide = durée de l'instance. */
  duration: string;
}

interface Bound {
  min: number;
  max: number;
}

type Fields = Record<string, string>;

/** Texte d'un code d'erreur du serveur, repli générique si la locale ne le connaît pas. */
export function errorText(code: string | undefined): string {
  const key = `errors.${code}`;
  return code && hasKey(key) ? t(key as MessageKey) : t("common.error");
}

const chars = (s: string) => [...s].length;

/** Champ numérique vide ou entier sûr : jamais de décimale ni d'exposant envoyé au serveur. */
const isIntOrEmpty = (s: string) => {
  const v = s.trim();
  return v === "" || (/^-?\d+$/.test(v) && Number.isSafeInteger(Number(v)));
};

/** Bornes par défaut d'une classe, ramenées à la taille de l'instance (comme le serveur). */
function defaultBound(defaultMin: number, defaultMax: number, cap: number): Bound {
  const max = Math.min(defaultMax, cap);
  return { min: Math.min(defaultMin, max), max };
}

interface Props {
  reference: ReferenceSet | null;
}

export function NewEvent({ reference }: Props) {
  const lang = useLang();
  const [created, setCreated] = useState<{ event: CreatedEvent; stored: boolean } | null>(null);
  const token = useRef<Promise<string> | null>(null);

  useEffect(() => {
    // Demandé au premier rendu, sans bloquer l'écran ; un échec est rattrapé à l'envoi.
    const p = getFormToken("create");
    p.catch(() => {});
    token.current = p;
  }, []);

  if (created) return <LinksCreated created={created.event} stored={created.stored} />;
  return (
    <Form
      reference={reference}
      lang={lang}
      token={() => token.current ?? Promise.reject(new Error("no token"))}
      onCreated={(event) => {
        // Gardé avant l'affichage : un rechargement de l'étape suivante ne perd rien de récupérable.
        const stored = saveOrganizerLinks(event.eventId, { organizer: event.organizerSecret, join: event.joinSecret });
        setCreated({ event, stored });
        window.scrollTo?.(0, 0);
      }}
    />
  );
}

interface FormProps {
  reference: ReferenceSet | null;
  lang: string;
  token: () => Promise<string>;
  onCreated: (event: CreatedEvent) => void;
}

function Form({ reference, lang, token, onCreated }: FormProps) {
  const ids = useId();
  const instances = reference?.instances.filter((i) => i.enabled) ?? [];
  const regions = reference?.regions.filter((r) => r.enabled) ?? [];
  const classes = reference?.classes.filter((c) => c.enabled) ?? [];

  const [instanceId, setInstanceId] = useState("");
  const [regionId, setRegionId] = useState("");
  const [scope, setScope] = useState<FactionScope>("all");
  const nextKey = useRef(1);
  const [slots, setSlots] = useState<SlotDraft[]>([{ key: 0, local: "", duration: "" }]);
  const [note, setNote] = useState("");
  const [discordUrl, setDiscordUrl] = useState("");
  const [listed, setListed] = useState(true);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [minCp, setMinCp] = useState("");
  const [bounds, setBounds] = useState<Record<string, Bound>>({});
  const [website, setWebsite] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fields, setFields] = useState<Fields>({});

  const instance: Instance | undefined = instances.find((i) => i.id === instanceId) ?? instances[0];
  const region = regions.find((r) => r.id === regionId) ?? regions[0];
  const cap = instance ? instance.parties * instance.partySize : 0;
  // Même règle qu'au changement d'instance : « Toutes » n'existe pas sur une instance mono-faction,
  // y compris pour l'instance par défaut, connue parfois après le premier rendu.
  const effectiveScope: FactionScope = scope === "all" && instance && !instance.crossFaction ? "elyos" : scope;
  const scopeAllowed = effectiveScope !== "all" || !!instance?.crossFaction;

  const boundOf = (id: string, defaultMin: number, defaultMax: number): Bound =>
    bounds[id] ?? defaultBound(defaultMin, defaultMax, cap);

  let reason: MessageKey | null = null;
  if (!instance || !region) reason = "new.reason.loading";
  else if (!scopeAllowed) reason = "new.reason.scope";
  else if (slots.some((s) => localInputToUtc(s.local) === null)) reason = "new.reason.slotDate";
  else if (chars(note) > NOTE_MAX) reason = "new.reason.note";
  else if (!isIntOrEmpty(minCp) || slots.some((s) => !isIntOrEmpty(s.duration))) reason = "new.reason.integer";

  const fieldError = (path: string): string | undefined => fields[path];
  const errorId = (path: string) => `${ids}-err-${path}`;
  const errorLine = (path: string, prefix = false): ReactNode => {
    const code = prefix
      ? Object.entries(fields).find(([k]) => k === path || k.startsWith(`${path}.`))?.[1]
      : fieldError(path);
    if (!code) return null;
    return (
      <p id={errorId(path)} className="mt-1 text-meta text-danger">
        {errorText(code)}
      </p>
    );
  };
  const invalid = (path: string) => (fields[path] ? { "aria-invalid": true, "aria-describedby": errorId(path) } : {});

  /** Retire les erreurs du serveur dont le chemin est `path` ou commence par `path.`. */
  function clearField(path: string) {
    setFields((all) => {
      const keys = Object.keys(all).filter((k) => k === path || k.startsWith(`${path}.`));
      if (keys.length === 0) return all;
      const next = { ...all };
      for (const k of keys) delete next[k];
      return next;
    });
  }

  function updateSlot(key: number, n: number, patch: Partial<SlotDraft>) {
    clearField(`slots.${n}.${patch.local !== undefined ? "startsAt" : "durationMin"}`);
    setSlots((all) => all.map((s) => (s.key === key ? { ...s, ...patch } : s)));
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (reason || sending || !instance || !region) return;
    const classBounds: Record<string, Bound> = {};
    for (const c of classes) if (bounds[c.id]) classBounds[c.id] = bounds[c.id];
    const cp = minCp.trim() === "" ? undefined : Number(minCp.trim());
    const input: CreateEvent = {
      instance: instance.id,
      region: region.id,
      factionScope: effectiveScope,
      slots: slots.map((s) => {
        const startsAt = localInputToUtc(s.local) ?? "";
        const d = s.duration.trim();
        return d === "" ? { startsAt } : { startsAt, durationMin: Number(d) };
      }),
      visibility: listed ? "listed" : "unlisted",
      ...(note !== "" && { note }),
      ...(discordUrl.trim() !== "" && { discordUrl: discordUrl.trim() }),
      ...(cp !== undefined && { minCp: cp }),
      ...(Object.keys(classBounds).length > 0 && { classBounds }),
    };
    setSending(true);
    setError(null);
    setFields({});
    try {
      let formToken: string;
      try {
        formToken = await token();
      } catch {
        formToken = await getFormToken("create");
      }
      const event = await createEvent(input, formToken, website);
      onCreated(event);
    } catch (err) {
      const api = err instanceof ApiError ? err : new ApiError(0);
      // Création en pause : message dédié de `/new`, le formulaire reste lisible (EXPERIENCE.md).
      setError(api.code === "limit.paused" ? t("new.paused") : errorText(api.code));
      setFields(api.fields);
      if (Object.keys(api.fields).some((k) => k === "minCp" || k.startsWith("classBounds"))) setOptionsOpen(true);
      setSending(false);
    }
  }

  const label = "block text-label";
  const control =
    "mt-1 block min-h-tap w-full min-w-0 rounded-md border border-border bg-surface-sunken px-3 text-body text-ink-primary aria-[invalid=true]:border-danger";
  const factions: FactionScope[] = ["all", "elyos", "asmodian"];

  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 className="text-title">{t("new.title")}</h1>
      <form noValidate onSubmit={submit} className="mt-4 flex flex-col gap-5">
        <div>
          <label htmlFor={`${ids}-instance`} className={label}>
            {t("new.instance")}
          </label>
          <select
            id={`${ids}-instance`}
            value={instance?.id ?? ""}
            onChange={(e) => {
              const next = instances.find((i) => i.id === e.target.value);
              setInstanceId(e.target.value);
              clearField("instance");
              if (next && !next.crossFaction && scope === "all") setScope("elyos");
              if (next) {
                // Bornes déjà réglées ramenées à la taille de la nouvelle instance.
                const nextCap = next.parties * next.partySize;
                setBounds((all) =>
                  Object.fromEntries(
                    Object.entries(all).map(([id, b]) => {
                      const max = Math.min(b.max, nextCap);
                      return [id, { min: Math.min(b.min, max), max }];
                    }),
                  ),
                );
              }
            }}
            className={control}
            {...invalid("instance")}
          >
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {labelOf(i.label, lang)}
              </option>
            ))}
          </select>
          {errorLine("instance")}
        </div>

        <div>
          <label htmlFor={`${ids}-region`} className={label}>
            {t("new.region")}
          </label>
          <select
            id={`${ids}-region`}
            value={region?.id ?? ""}
            onChange={(e) => {
              setRegionId(e.target.value);
              clearField("region");
            }}
            className={control}
            {...invalid("region")}
          >
            {regions.map((r) => (
              <option key={r.id} value={r.id}>
                {labelOf(r.label, lang)}
              </option>
            ))}
          </select>
          {errorLine("region")}
        </div>

        <fieldset {...invalid("factionScope")}>
          <legend className={label}>{t("new.scope")}</legend>
          <div className="mt-1 grid grid-cols-3 gap-2">
            {factions.map((f) => {
              const disabled = f === "all" && !!instance && !instance.crossFaction;
              return (
                <label
                  key={f}
                  className={`flex min-h-tap items-center justify-center rounded-md border px-2 text-center text-label has-checked:border-2 has-checked:border-accent has-focus-visible:outline-2 has-focus-visible:outline-accent ${
                    disabled ? "border-border text-ink-disabled" : "border-border text-ink-primary"
                  }`}
                >
                  <input
                    type="radio"
                    name={`${ids}-scope`}
                    value={f}
                    checked={effectiveScope === f}
                    disabled={disabled}
                    onChange={() => {
                      setScope(f);
                      clearField("factionScope");
                    }}
                    className="sr-only"
                  />
                  {f === "all" ? t("new.scope.all") : reference ? labelOf(reference.factions[f], lang) : f}
                </label>
              );
            })}
          </div>
          {errorLine("factionScope")}
        </fieldset>

        <fieldset>
          <legend className={label}>{t("new.slots")}</legend>
          <p className="text-meta text-ink-secondary">{t("new.timezone", { zone: timeZoneLabel(lang) })}</p>
          <ol className="mt-2 flex flex-col gap-3">
            {slots.map((s, n) => (
              <li key={s.key} data-testid={`slot-${n}`}>
                <div className="flex items-center gap-2">
                  <input
                    type="datetime-local"
                    aria-label={t("new.slot.date", { n: n + 1 })}
                    value={s.local}
                    onChange={(e) => updateSlot(s.key, n, { local: e.target.value })}
                    className={`${control} mt-0 flex-1 px-2`}
                    {...invalid(`slots.${n}.startsAt`)}
                  />
                  <input
                    type="number"
                    inputMode="numeric"
                    min={15}
                    max={720}
                    aria-label={t("new.slot.duration", { n: n + 1 })}
                    value={s.duration}
                    placeholder={instance ? String(instance.durationMin) : ""}
                    onChange={(e) => updateSlot(s.key, n, { duration: e.target.value })}
                    className={`${control} mt-0 w-16 shrink-0 px-2 text-center tabular-nums`}
                    {...invalid(`slots.${n}.durationMin`)}
                  />
                  <span className="text-meta text-ink-secondary">{t("new.slot.minutes")}</span>
                  {slots.length > 1 && (
                    <button
                      type="button"
                      aria-label={t("new.slot.remove", { n: n + 1 })}
                      onClick={() => {
                        // Les erreurs sont indexées par rang : après un retrait, elles ne désignent plus le bon créneau.
                        clearField("slots");
                        setSlots((all) => all.filter((x) => x.key !== s.key));
                      }}
                      className="flex size-tap shrink-0 items-center justify-center rounded-md border border-border text-ink-secondary"
                    >
                      ×
                    </button>
                  )}
                </div>
                {errorLine(`slots.${n}.startsAt`)}
                {errorLine(`slots.${n}.durationMin`)}
              </li>
            ))}
          </ol>
          {errorLine("slots")}
          <button
            type="button"
            disabled={slots.length >= SLOTS_MAX}
            onClick={() => {
              setFields((all) => {
                if (!("slots" in all)) return all;
                const rest = { ...all };
                delete rest.slots;
                return rest;
              });
              setSlots((all) => [...all, { key: nextKey.current++, local: "", duration: "" }]);
            }}
            className="mt-3 min-h-tap rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
          >
            {t("new.slot.add")}
          </button>
        </fieldset>

        <div>
          <div className="flex items-baseline justify-between gap-2">
            <label htmlFor={`${ids}-note`} className={label}>
              {t("new.note")}
            </label>
            <span className={`text-meta tabular-nums ${chars(note) > NOTE_MAX ? "text-danger" : "text-ink-secondary"}`}>
              {t("new.noteCount", { count: chars(note), max: NOTE_MAX })}
            </span>
          </div>
          <textarea
            id={`${ids}-note`}
            rows={3}
            value={note}
            onChange={(e) => {
              setNote(e.target.value);
              clearField("note");
            }}
            className={`${control} py-2`}
            {...invalid("note")}
          />
          {errorLine("note")}
        </div>

        <div>
          <label htmlFor={`${ids}-discord`} className={label}>
            {t("new.discord")}
          </label>
          <input
            id={`${ids}-discord`}
            type="url"
            inputMode="url"
            autoComplete="off"
            placeholder="https://discord.gg/…"
            value={discordUrl}
            onChange={(e) => {
              setDiscordUrl(e.target.value);
              clearField("discordUrl");
            }}
            className={control}
            {...invalid("discordUrl")}
          />
          {errorLine("discordUrl")}
        </div>

        <label className="flex min-h-tap items-center gap-3 text-body">
          <input
            type="checkbox"
            checked={listed}
            onChange={(e) => setListed(e.target.checked)}
            className="size-5 accent-[var(--accent)]"
          />
          {t("new.listed")}
        </label>

        <details
          open={optionsOpen}
          onToggle={(e) => setOptionsOpen((e.currentTarget as HTMLDetailsElement).open)}
          className="rounded-md border border-border px-3"
        >
          <summary className="flex min-h-tap cursor-pointer items-center text-label">{t("new.options")}</summary>
          <div className="flex flex-col gap-4 pb-4">
            <div>
              <label htmlFor={`${ids}-mincp`} className={label}>
                {t("new.minCp")}
              </label>
              <input
                id={`${ids}-mincp`}
                type="number"
                inputMode="numeric"
                min={0}
                max={999999}
                value={minCp}
                onChange={(e) => {
                  setMinCp(e.target.value);
                  clearField("minCp");
                }}
                className={`${control} tabular-nums`}
                {...invalid("minCp")}
              />
              {errorLine("minCp")}
            </div>
            <div>
              <p className={label}>{t("new.bounds")}</p>
              <ul className="mt-2 flex flex-col gap-3">
                {classes.map((c) => {
                  const b = boundOf(c.id, c.defaultMin, c.defaultMax);
                  const name = labelOf(c.label, lang);
                  const path = `classBounds.${c.id}`;
                  const bad = Object.keys(fields).some((k) => k === path || k.startsWith(`${path}.`));
                  return (
                    <li key={c.id} data-testid={`bound-${c.id}`}>
                      <p className="text-body">{name}</p>
                      <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-2">
                        <span className="flex items-center gap-2">
                          <span className="w-7 text-meta text-ink-secondary">{t("new.bound.min")}</span>
                          <Stepper
                            value={b.min}
                            min={0}
                            max={b.max}
                            label={t("new.bound.minOf", { class: name })}
                            invalid={bad}
                            describedBy={bad ? errorId(path) : undefined}
                            onChange={(min) => {
                              clearField(path);
                              setBounds((all) => ({ ...all, [c.id]: { ...b, min } }));
                            }}
                          />
                        </span>
                        <span className="flex items-center gap-2">
                          <span className="w-7 text-meta text-ink-secondary">{t("new.bound.max")}</span>
                          <Stepper
                            value={b.max}
                            min={b.min}
                            max={Math.max(cap, b.min)}
                            label={t("new.bound.maxOf", { class: name })}
                            invalid={bad}
                            describedBy={bad ? errorId(path) : undefined}
                            onChange={(max) => {
                              clearField(path);
                              setBounds((all) => ({ ...all, [c.id]: { ...b, max } }));
                            }}
                          />
                        </span>
                      </div>
                      {errorLine(path, true)}
                    </li>
                  );
                })}
              </ul>
            </div>
          </div>
        </details>

        {/* Honeypot : invisible et hors du parcours clavier, laissé vide par un humain. */}
        <div aria-hidden="true" className="absolute -left-[9999px] h-px w-px overflow-hidden">
          <label>
            {t("new.honeypot")}
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
            {sending ? t("new.sending") : t("new.publish")}
          </button>
          {reason && (
            <p id={`${ids}-reason`} className="mt-2 text-meta text-ink-secondary">
              {t(reason, { max: NOTE_MAX })}
            </p>
          )}
          {error && (
            <p role="alert" className="mt-2 text-label text-danger">
              {error}
            </p>
          )}
        </div>
      </form>
    </main>
  );
}
