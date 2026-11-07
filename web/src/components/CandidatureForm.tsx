// Formulaire de candidature (CAP-3, EXPERIENCE.md *Formulaires*), partagé par la candidature
// (`/e`, `/j`) et par la modification sur Ma candidature (`/p`). Pseudo, serveur groupé par faction et
// filtré par région et portée, classe de l'événement, CP, puis une bascule ✓ par créneau à venir avec
// ☆ pour la préférence. Faction et rôle s'affichent en lecture seule après le choix. Le formulaire ne
// connaît ni jeton ni URL : `onSubmit` envoie et lève une `ApiError` en cas de refus, dont les
// `fields` s'affichent sous les champs, saisie conservée. Libellés de jeu lus dans la référence (AD-6).

import { useId, useState, type FormEvent, type ReactNode } from "react";
import { ApiError } from "../api/client";
import { labelOf } from "../api/reference";
import type { Faction, GameClass, PublicEvent, ReferenceSet, Server, SubmitCandidature } from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import { formatDay, formatSlot } from "../lib/time";
import { errorText } from "../lib/form";
import { FactionDot, roleLabel } from "./EventSummary";

export const PSEUDO_MAX = 16;
export const CP_MAX = 999_999;

const FACTIONS: Faction[] = ["elyos", "asmodian"];

/** Même règle que `domain::validate::pseudo` : 1 à 16 lettres ou chiffres, toute écriture. */
export function isPseudo(s: string): boolean {
  return [...s].length <= PSEUDO_MAX && /^[\p{Alphabetic}\p{N}]+$/u.test(s);
}

function isCp(s: string): boolean {
  return /^\d+$/.test(s.trim()) && Number(s.trim()) <= CP_MAX;
}

/** Saisie de départ ; `checked` associe chaque créneau coché à sa préférence. */
export interface CandidatureValues {
  pseudo?: string;
  server?: string;
  class?: string;
  cp?: string;
  checked?: Record<string, boolean>;
}

export interface CandidatureFormProps {
  event: PublicEvent;
  reference: ReferenceSet | null;
  now: () => number;
  initial?: CandidatureValues;
  /** Serveur et classe déjà enregistrés : toujours proposés, même désactivés depuis (AD-6). */
  current?: { server: string; class: string };
  /** Disponibilités gardées sur des créneaux passés : elles comptent pour « au moins un créneau ». */
  kept?: number;
  title: string;
  /** Ligne sous le titre (mode lien direct). */
  intro?: ReactNode;
  submitLabel: string;
  /** Champ piège pour les robots, seulement sur un formulaire public. */
  honeypot?: boolean;
  /** Envoie la saisie ; une `ApiError` levée s'affiche, la saisie reste. */
  onSubmit: (input: SubmitCandidature, website: string) => Promise<void>;
  /** Bouton secondaire « Annuler », absent si non fourni. */
  onCancel?: () => void;
  cancelLabel?: string;
}

type Fields = Record<string, string>;

export function CandidatureForm({
  event,
  reference,
  now,
  initial = {},
  current,
  kept = 0,
  title,
  intro,
  submitLabel,
  honeypot = false,
  onSubmit,
  onCancel,
  cancelLabel,
}: CandidatureFormProps) {
  const lang = useLang();
  const ids = useId();
  const [pseudo, setPseudo] = useState(initial.pseudo ?? "");
  const [serverId, setServerId] = useState(initial.server ?? "");
  const [classId, setClassId] = useState(initial.class ?? "");
  const [cp, setCp] = useState(initial.cp ?? "");
  /** Créneaux cochés → préféré. */
  const [checked, setChecked] = useState<Record<string, boolean>>(initial.checked ?? {});
  const [website, setWebsite] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fields, setFields] = useState<Fields>({});
  /** Créneaux envoyés, dans l'ordre : `availabilities.N.slotId` désigne le N-ième. */
  const [submitted, setSubmitted] = useState<string[]>([]);

  const servers: Server[] =
    reference?.servers.filter(
      (s) =>
        (s.enabled || s.id === current?.server) &&
        s.region === event.region &&
        (event.factionScope === "all" || s.faction === event.factionScope),
    ) ?? [];
  const classes: GameClass[] =
    reference?.classes.filter((c) => (c.enabled || c.id === current?.class) && c.id in event.countsByClass) ?? [];
  const server = servers.find((s) => s.id === serverId);
  const gameClass = classes.find((c) => c.id === classId);
  // Valeur enregistrée absente de la référence (AD-6) : gardée telle quelle, libellée par son id.
  const missingServer = reference && current && !reference.servers.some((s) => s.id === current.server) ? current.server : null;
  const missingClass = reference && current && !reference.classes.some((c) => c.id === current.class) ? current.class : null;
  const serverValue = server?.id ?? (serverId !== "" && serverId === missingServer ? serverId : "");
  const classValue = gameClass?.id ?? (classId !== "" && classId === missingClass ? classId : "");
  const upcoming = event.slots.filter((s) => Date.parse(s.startsAt) > now());
  const picked = upcoming.filter((s) => s.id in checked);
  const cpValue = cp.trim();
  const belowMin = isCp(cpValue) && event.minCp !== undefined && Number(cpValue) < event.minCp;

  let reason: MessageKey | null = null;
  if (!reference) reason = "event.reason.loading";
  else if (!isPseudo(pseudo)) reason = "event.reason.pseudo";
  else if (!serverValue) reason = "event.reason.server";
  else if (!classValue) reason = "event.reason.class";
  else if (!isCp(cpValue)) reason = "event.reason.cp";
  else if (picked.length + kept === 0) reason = "event.reason.slots";

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
    if (reason || sending || !serverValue || !classValue) return;
    const input: SubmitCandidature = {
      pseudo,
      server: serverValue,
      class: classValue,
      cp: Number(cpValue),
      availabilities: picked.map((s) => ({ slotId: s.id, preferred: checked[s.id] === true })),
    };
    setSending(true);
    setError(null);
    setFields({});
    setSubmitted(input.availabilities.map((a) => a.slotId));
    try {
      // Un succès démonte le formulaire (navigation ou retour à la lecture).
      await onSubmit(input, website);
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
        {title}
      </h2>
      {intro}

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
          value={serverValue}
          onChange={(e) => {
            setServerId(e.target.value);
            clearField("server");
          }}
          className={control}
          {...invalid("server")}
        >
          <option value="">{t("event.server.choose")}</option>
          {missingServer && <option value={missingServer}>{missingServer}</option>}
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
          value={classValue}
          onChange={(e) => {
            setClassId(e.target.value);
            clearField("class");
          }}
          className={control}
          {...invalid("class")}
        >
          <option value="">{t("event.class.choose")}</option>
          {missingClass && <option value={missingClass}>{missingClass}</option>}
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
      {honeypot && (
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
      )}

      <div>
        <button
          type="submit"
          disabled={!!reason || sending}
          aria-describedby={reason ? `${ids}-reason` : undefined}
          className="min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
        >
          {sending ? t("event.sending") : submitLabel}
        </button>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            disabled={sending}
            className="mt-2 min-h-tap w-full rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
          >
            {cancelLabel}
          </button>
        )}
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
