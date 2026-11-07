// Écran `/new` (CAP-1, EXPERIENCE.md *Formulaires*) : un seul écran à 360 px, puis l'étape « Liens
// créés ». Le jeton de formulaire est demandé dès le premier rendu, en parallèle (AD-19). Les heures
// sont saisies en heure locale et envoyées en UTC (AD-11). Libellés de jeu lus dans la référence.
// « Dupliquer » (`/o`, Réglages) arrive avec un brouillon dans l'état de navigation : le formulaire
// en part, sauf les dates, avec les règles de toute saisie (instance ou région désactivée ignorée,
// bornes ramenées à l'instance, portée ajustée). L'état est effacé dès sa lecture : un rechargement
// rend un formulaire vierge.

import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { useLocation, useNavigate } from "react-router";
import { ApiError } from "../api/client";
import { createEvent, getFormToken } from "../api/events";
import { saveOrganizerLinks } from "../api/links";
import { labelOf } from "../api/reference";
import type { CreateEvent, CreatedEvent, FactionScope, Instance, ReferenceSet } from "../api/types";
import { BoundsEditor } from "../components/BoundsEditor";
import {
  DiscordField,
  ListedField,
  MinCpField,
  NoteField,
  controlClass,
  labelClass,
} from "../components/EventFields";
import { SlotInputs } from "../components/SlotEditor";
import { t, useLang, type MessageKey } from "../i18n";
import { duplicateFromState, type DuplicateDraft } from "../lib/duplicate";
import {
  NOTE_MAX,
  SLOTS_MAX,
  capOf,
  chars,
  clampBounds,
  defaultBound,
  errorText,
  isIntOrEmpty,
  useFieldErrors,
  type Bound,
} from "../lib/form";
import { localInputToUtc, timeZoneLabel } from "../lib/time";
import { LinksCreated } from "./LinksCreated";

interface SlotDraft {
  key: number;
  /** Valeur de `datetime-local`, heure locale. */
  local: string;
  /** Vide = durée de l'instance. */
  duration: string;
  /** Durée illisible pour le navigateur (`validity.badInput`), rapportée vide. */
  durationBad?: boolean;
}

interface Props {
  reference: ReferenceSet | null;
}

/** Vrai si un brouillon de « Dupliquer » porte un CP minimum ou des bornes autres que celles par
 * défaut, une fois ramenées à l'instance retenue : « Options » s'ouvre alors, pour que rien ne se
 * publie sans avoir été vu. Sans la référence, toute borne compte. */
function carriesOptions(d: DuplicateDraft, reference: ReferenceSet | null): boolean {
  if (d.minCp !== undefined) return true;
  const instances = reference?.instances.filter((i) => i.enabled) ?? [];
  const instance = instances.find((i) => i.id === d.instance) ?? instances[0];
  if (!reference || !instance) return Object.keys(d.classBounds).length > 0;
  const cap = capOf(instance);
  return reference.classes.some((c) => {
    const b = d.classBounds[c.id];
    if (!c.enabled || !b) return false;
    const shown = defaultBound(b.min, b.max, cap);
    const usual = defaultBound(c.defaultMin, c.defaultMax, cap);
    return shown.min !== usual.min || shown.max !== usual.max;
  });
}

export function NewEvent({ reference }: Props) {
  const lang = useLang();
  const location = useLocation();
  const navigate = useNavigate();
  // Brouillon de « Dupliquer », lu une fois au montage.
  const [seed] = useState(() => duplicateFromState(location.state));
  const [created, setCreated] = useState<{ event: CreatedEvent; stored: boolean } | null>(null);
  const token = useRef<Promise<string> | null>(null);

  useEffect(() => {
    // Consommé : effacé de l'historique, pour qu'un rechargement rende un formulaire vierge.
    if (location.state !== null && location.state !== undefined) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

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
      initial={seed}
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
  /** Brouillon de « Dupliquer » ; absent = formulaire vierge. */
  initial: DuplicateDraft | null;
  token: () => Promise<string>;
  onCreated: (event: CreatedEvent) => void;
}

function Form({ reference, lang, initial, token, onCreated }: FormProps) {
  const ids = useId();
  const instances = reference?.instances.filter((i) => i.enabled) ?? [];
  const regions = reference?.regions.filter((r) => r.enabled) ?? [];
  const classes = reference?.classes.filter((c) => c.enabled) ?? [];

  // Valeurs du brouillon telles quelles : une instance ou une région absente des choix retombe sur
  // le premier choix, comme toute valeur inconnue, et la portée suit l'instance retenue.
  const [instanceId, setInstanceId] = useState(initial?.instance ?? "");
  const [regionId, setRegionId] = useState(initial?.region ?? "");
  const [scope, setScope] = useState<FactionScope>(initial?.factionScope ?? "all");
  const nextKey = useRef(initial ? initial.slots.length : 1);
  const [slots, setSlots] = useState<SlotDraft[]>(() =>
    initial
      ? initial.slots.map((s, key) => ({
          key,
          local: "",
          duration: s.durationMin === undefined ? "" : String(s.durationMin),
        }))
      : [{ key: 0, local: "", duration: "" }],
  );
  const [note, setNote] = useState(initial?.note ?? "");
  const [discordUrl, setDiscordUrl] = useState(initial?.discordUrl ?? "");
  const [listed, setListed] = useState(initial ? initial.visibility === "listed" : true);
  const [optionsOpen, setOptionsOpen] = useState(() => initial !== null && carriesOptions(initial, reference));
  const [minCp, setMinCp] = useState(initial?.minCp === undefined ? "" : String(initial.minCp));
  const [minCpBad, setMinCpBad] = useState(false);
  const [bounds, setBounds] = useState<Record<string, Bound>>(initial?.classBounds ?? {});
  const [website, setWebsite] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const errors = useFieldErrors(ids);
  const { setFields, errorLine, invalid, clearField } = errors;

  const instance: Instance | undefined = instances.find((i) => i.id === instanceId) ?? instances[0];
  const region = regions.find((r) => r.id === regionId) ?? regions[0];
  const cap = instance ? capOf(instance) : 0;
  // Même règle qu'au changement d'instance : « Toutes » n'existe pas sur une instance mono-faction,
  // y compris pour l'instance par défaut, connue parfois après le premier rendu.
  const effectiveScope: FactionScope = scope === "all" && instance && !instance.crossFaction ? "elyos" : scope;
  const scopeAllowed = effectiveScope !== "all" || !!instance?.crossFaction;

  // Toujours dans la taille de l'instance retenue : une borne reprise d'un raid dupliqué peut la
  // dépasser, l'instance d'origine pouvant être désactivée ou la référence arrivée après.
  const boundOf = (id: string, defaultMin: number, defaultMax: number): Bound => {
    const b = bounds[id];
    return b ? defaultBound(b.min, b.max, cap) : defaultBound(defaultMin, defaultMax, cap);
  };

  let reason: MessageKey | null = null;
  if (!instance || !region) reason = "new.reason.loading";
  else if (!scopeAllowed) reason = "new.reason.scope";
  else if (slots.some((s) => localInputToUtc(s.local) === null)) reason = "new.reason.slotDate";
  else if (chars(note) > NOTE_MAX) reason = "new.reason.note";
  else if (minCpBad || !isIntOrEmpty(minCp) || slots.some((s) => s.durationBad || !isIntOrEmpty(s.duration))) {
    reason = "new.reason.integer";
  }

  function updateSlot(key: number, n: number, patch: Partial<SlotDraft>) {
    clearField(`slots.${n}.${patch.local !== undefined ? "startsAt" : "durationMin"}`);
    setSlots((all) => all.map((s) => (s.key === key ? { ...s, ...patch } : s)));
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (reason || sending || !instance || !region) return;
    const classBounds: Record<string, Bound> = {};
    for (const c of classes) if (bounds[c.id]) classBounds[c.id] = boundOf(c.id, c.defaultMin, c.defaultMax);
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

  const label = labelClass;
  const control = controlClass;
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
              // Bornes déjà réglées ramenées à la taille de la nouvelle instance.
              if (next) setBounds((all) => clampBounds(all, capOf(next)));
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
                <SlotInputs
                  local={s.local}
                  duration={s.duration}
                  instanceDuration={instance?.durationMin}
                  dateLabel={t("new.slot.date", { n: n + 1 })}
                  durationLabel={t("new.slot.duration", { n: n + 1 })}
                  onLocal={(local) => updateSlot(s.key, n, { local })}
                  onDuration={(duration, durationBad) => updateSlot(s.key, n, { duration, durationBad })}
                  dateAria={invalid(`slots.${n}.startsAt`)}
                  durationAria={invalid(`slots.${n}.durationMin`)}
                >
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
                </SlotInputs>
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

        <NoteField id={`${ids}-note`} value={note} onChange={setNote} errors={errors} />
        <DiscordField id={`${ids}-discord`} value={discordUrl} onChange={setDiscordUrl} errors={errors} />
        <ListedField checked={listed} onChange={setListed} />

        <details
          open={optionsOpen}
          onToggle={(e) => setOptionsOpen((e.currentTarget as HTMLDetailsElement).open)}
          className="rounded-md border border-border px-3"
        >
          <summary className="flex min-h-tap cursor-pointer items-center text-label">{t("new.options")}</summary>
          <div className="flex flex-col gap-4 pb-4">
            <MinCpField
              id={`${ids}-mincp`}
              value={minCp}
              onChange={(v, bad) => {
                setMinCp(v);
                setMinCpBad(bad);
              }}
              errors={errors}
            />
            <BoundsEditor
              rows={classes.map((c) => ({
                id: c.id,
                name: labelOf(c.label, lang),
                bound: boundOf(c.id, c.defaultMin, c.defaultMax),
              }))}
              cap={cap}
              errors={errors}
              onChange={(id, b) => setBounds((all) => ({ ...all, [id]: b }))}
            />
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
