// Onglet Réglages de `/o/<secret>` (CAP-5, AD-4 *Mutabilité*, EXPERIENCE.md *Component Patterns*).
//
// - « Le raid » : instance, région et portée, modifiables tant qu'aucune candidature n'existe, puis
//   en lecture avec « Fixés depuis la première candidature. ».
// - Les réglages toujours modifiables : note, lien Discord, « Lister publiquement », CP minimum et
//   bornes par classe, enregistrés d'un seul bouton principal « Enregistrer », désactivé avec sa
//   raison tant que rien n'a changé ou que la saisie est invalide.
// - « Créneaux », en `open` seulement : suppression (confirmée si des joueurs l'ont coché) et ajout.
// - « Annuler le raid », confirmé, en bas.
//
// Le brouillon part de la vue et la suit tant qu'il n'a pas de changement non enregistré ; il repart
// de la vue que rend « Enregistrer ». Raid annulé : lecture seule, aucune action. Les commandes et la
// relecture passent par la page (séquencement, refus d'état), qui rend l'issue de chaque geste.

import { useEffect, useId, useRef, useState, type FormEvent, type RefObject } from "react";
import { labelOf } from "../api/reference";
import type {
  FactionScope,
  NewSlot,
  OrganizerEvent,
  PublicSlot,
  ReferenceSet,
  UpdateEventSettings,
} from "../api/types";
import { t, useLang, type MessageKey } from "../i18n";
import {
  NOTE_MAX,
  SLOTS_MAX,
  capOf,
  chars,
  clampBounds,
  errorText,
  isIntOrEmpty,
  useFieldErrors,
  type Bound,
  type Fields,
} from "../lib/form";
import { formatDay, formatSlot, localInputToUtc, timeZoneLabel } from "../lib/time";
import { BoundsEditor } from "./BoundsEditor";
import { ConfirmSheet } from "./ConfirmSheet";
import { DiscordField, ListedField, MinCpField, NoteField, controlClass, labelClass } from "./EventFields";
import { FactionDot, Summary } from "./EventSummary";
import { SlotInputs } from "./SlotEditor";

/** Commande de cet onglet. */
export type SettingsCommand = "settings" | "addSlot" | "removeSlot" | "cancel";

/** Issue d'une commande, rendue par la page. `handled` : la page a déjà traité le refus (lien mort,
 * code d'état affiché et vue relue, commande déjà en vol) ; sinon l'onglet le montre près du geste. */
export type CommandOutcome =
  | { ok: true; view: OrganizerEvent }
  | { ok: false; handled: boolean; code?: string; fields: Fields };

export interface SettingsActions {
  save: (body: UpdateEventSettings) => Promise<CommandOutcome>;
  addSlot: (slot: NewSlot) => Promise<CommandOutcome>;
  removeSlot: (slotId: string) => Promise<CommandOutcome>;
  cancel: () => Promise<CommandOutcome>;
}

interface Props {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  now: () => number;
  /** Commande en vol, de cet onglet ou d'un autre : aucun envoi en attendant. */
  busy: string | null;
  actions: SettingsActions;
  /** Repli du focus quand le déclencheur d'une feuille a disparu. */
  fallbackFocus: RefObject<HTMLElement | null>;
}

export function EventSettings(props: Props) {
  if (props.view.event.status === "cancelled") return <ReadOnly {...props} />;
  return <Editor {...props} />;
}

/** Raid annulé : le résumé et le lien Discord, sans aucun champ ni bouton. */
function ReadOnly({ view, reference, now }: Props) {
  useLang();
  return (
    <div data-testid="settings" className="mt-4">
      <Summary event={view.event} reference={reference} now={now} heading="h2" />
      <h3 className="mt-6 text-meta text-ink-secondary">{t("organizer.discord")}</h3>
      {view.discordUrl ? (
        <a
          href={view.discordUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="mt-1 block text-body break-all text-ink-primary"
        >
          {view.discordUrl}
        </a>
      ) : (
        <p className="mt-1 text-body text-ink-secondary">{t("organizer.noDiscord")}</p>
      )}
    </div>
  );
}

/** Saisie de l'onglet, telle qu'envoyée. */
interface Draft {
  instance: string;
  region: string;
  factionScope: FactionScope;
  note: string;
  discordUrl: string;
  listed: boolean;
  minCp: string;
  /** Le champ CP contient une saisie que le navigateur ne sait pas lire (`validity.badInput`) : sa
   * valeur rapportée est vide, mais ce n'est pas un effacement. */
  minCpBad: boolean;
  bounds: Record<string, Bound>;
}

function fromView(v: OrganizerEvent): Draft {
  return {
    instance: v.event.instance,
    region: v.event.region,
    factionScope: v.event.factionScope,
    note: v.event.note,
    discordUrl: v.discordUrl ?? "",
    listed: v.event.visibility === "listed",
    minCp: v.event.minCp === undefined ? "" : String(v.event.minCp),
    minCpBad: false,
    bounds: v.classBounds,
  };
}

function sameDraft(a: Draft, b: Draft): boolean {
  const ids = Object.keys(a.bounds);
  return (
    a.instance === b.instance &&
    a.region === b.region &&
    a.factionScope === b.factionScope &&
    a.note === b.note &&
    a.discordUrl.trim() === b.discordUrl.trim() &&
    a.listed === b.listed &&
    a.minCp.trim() === b.minCp.trim() &&
    a.minCpBad === b.minCpBad &&
    ids.length === Object.keys(b.bounds).length &&
    ids.every((id) => b.bounds[id]?.min === a.bounds[id].min && b.bounds[id]?.max === a.bounds[id].max)
  );
}

function Editor({ view, reference, now, busy, actions, fallbackFocus }: Props) {
  const lang = useLang();
  const ids = useId();
  const errors = useFieldErrors(`${ids}-settings`);
  const slotErrors = useFieldErrors(`${ids}-slot`);
  const [draft, setDraft] = useState<Draft>(() => fromView(view));
  /** Vue dont le brouillon est parti, ou qu'il a suivie en dernier. */
  const synced = useRef<Draft>(draft);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [addLocal, setAddLocal] = useState("");
  const [addDuration, setAddDuration] = useState("");
  const [addDurationBad, setAddDurationBad] = useState(false);
  const [slotError, setSlotError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<{ slot: PublicSlot; ticked: number } | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [cancelError, setCancelError] = useState<string | null>(null);
  const removeTrigger = useRef<HTMLElement | null>(null);
  const cancelTrigger = useRef<HTMLButtonElement>(null);
  const slotsHeading = useRef<HTMLHeadingElement>(null);

  // Rafraîchissement : le brouillon suit la vue tant qu'il n'a pas de changement non enregistré.
  useEffect(() => {
    const next = fromView(view);
    const previous = synced.current;
    synced.current = next;
    setDraft((d) => (sameDraft(d, previous) ? next : d));
  }, [view]);

  const open = view.event.status === "open";
  // Créneau disparu, ou raid qui n'est plus `open` : la confirmation n'a plus d'objet.
  useEffect(() => {
    if (removing && (!open || !view.event.slots.some((s) => s.id === removing.slot.id))) setRemoving(null);
  }, [removing, open, view]);

  const frozen = view.candidatures.length > 0;
  // Champs figés : la vue fait foi, quel que soit le brouillon d'avant la première candidature. Des
  // bornes ramenées à une instance abandonnée repartent de la vue, sans compter comme un changement.
  const effective: Draft = frozen
    ? {
        ...draft,
        instance: view.event.instance,
        region: view.event.region,
        factionScope: view.event.factionScope,
        ...(draft.instance !== view.event.instance && { bounds: view.classBounds }),
      }
    : draft;
  const changed = !sameDraft(effective, fromView(view));
  // « Existe et `enabled` » ne vaut que pour une valeur changée (AD-6) : la valeur courante reste
  // proposée même retirée de la référence, et absente de la référence, sous son id.
  const instances = reference?.instances.filter((i) => i.enabled || i.id === view.event.instance) ?? [];
  const regions = reference?.regions.filter((r) => r.enabled || r.id === view.event.region) ?? [];
  const missingInstance = reference?.instances.some((i) => i.id === view.event.instance) ? null : view.event.instance;
  const missingRegion = reference?.regions.some((r) => r.id === view.event.region) ? null : view.event.region;
  const instance = reference?.instances.find((i) => i.id === effective.instance);
  // Instance inconnue de la référence : le plafond se lit dans les bornes enregistrées, jamais dans
  // le brouillon qu'il borne.
  const cap = instance ? capOf(instance) : Math.max(0, ...Object.values(view.classBounds).map((b) => b.max));
  // L'ajout de créneau prend la durée de l'instance enregistrée, comme le serveur.
  const storedInstance = reference?.instances.find((i) => i.id === view.event.instance);
  const raidChanged = effective.instance !== view.event.instance || effective.factionScope !== view.event.factionScope;
  const scopeAllowed = !raidChanged || effective.factionScope !== "all" || !instance || instance.crossFaction;

  let reason: MessageKey | null = null;
  if (!reference) reason = "settings.reason.loading";
  else if (effective.minCpBad || !isIntOrEmpty(effective.minCp)) reason = "settings.reason.integer";
  else if (!changed) reason = "settings.reason.unchanged";
  else if (!scopeAllowed) reason = "settings.reason.scope";
  else if (chars(effective.note) > NOTE_MAX) reason = "settings.reason.note";

  const patch = (p: Partial<Draft>) => setDraft((d) => ({ ...d, ...p }));

  function setInstance(id: string) {
    const next = reference?.instances.find((i) => i.id === id);
    errors.clearField("instance");
    setDraft((d) => ({
      ...d,
      instance: id,
      factionScope: next && !next.crossFaction && d.factionScope === "all" ? "elyos" : d.factionScope,
      // Bornes ramenées à la taille de la nouvelle instance, comme à la création.
      bounds: next ? clampBounds(d.bounds, capOf(next)) : d.bounds,
    }));
  }

  async function save(e: FormEvent) {
    e.preventDefault();
    if (reason || busy !== null) return;
    const cp = effective.minCp.trim();
    const discord = effective.discordUrl.trim();
    const body: UpdateEventSettings = {
      instance: effective.instance,
      region: effective.region,
      factionScope: effective.factionScope,
      note: effective.note,
      visibility: effective.listed ? "listed" : "unlisted",
      classBounds: effective.bounds,
      ...(discord !== "" && { discordUrl: discord }),
      ...(cp !== "" && { minCp: Number(cp) }),
    };
    setSaveError(null);
    errors.setFields({});
    const outcome = await actions.save(body);
    if (outcome.ok) {
      // Le brouillon repart de la vue rendue par la commande.
      const next = fromView(outcome.view);
      synced.current = next;
      setDraft(next);
    } else if (!outcome.handled) {
      setSaveError(errorText(outcome.code));
      errors.setFields(outcome.fields);
    }
  }

  const addStart = localInputToUtc(addLocal);
  let addReason: MessageKey | null = null;
  if (view.event.slots.length >= SLOTS_MAX) addReason = "settings.slot.max";
  else if (addDurationBad || !isIntOrEmpty(addDuration)) addReason = "settings.slot.integer";
  const canAdd = !addReason && addStart !== null && busy === null;

  async function add() {
    if (!canAdd || addStart === null) return;
    const d = addDuration.trim();
    setSlotError(null);
    slotErrors.setFields({});
    const outcome = await actions.addSlot(d === "" ? { startsAt: addStart } : { startsAt: addStart, durationMin: Number(d) });
    if (outcome.ok) {
      setAddLocal("");
      setAddDuration("");
      setAddDurationBad(false);
    } else if (!outcome.handled) {
      slotErrors.setFields(outcome.fields);
      if (Object.keys(outcome.fields).length === 0) setSlotError(errorText(outcome.code));
    }
  }

  /** Joueurs dont la dispo partirait avec le créneau : candidatures non retirées qui l'ont coché. */
  const tickedBy = (slotId: string) =>
    view.candidatures.filter((c) => c.status !== "withdrawn" && c.availabilities.some((a) => a.slotId === slotId))
      .length;

  function askRemove(slot: PublicSlot, from: HTMLButtonElement) {
    setSlotError(null);
    const ticked = tickedBy(slot.id);
    if (ticked === 0) {
      void remove(slot.id, false);
      return;
    }
    removeTrigger.current = from;
    setRemoveError(null);
    setRemoving({ slot, ticked });
  }

  async function remove(slotId: string, confirmed: boolean) {
    const outcome = await actions.removeSlot(slotId);
    if (outcome.ok || outcome.handled) {
      setRemoving(null);
      // Le bouton touché est parti avec son créneau : le focus revient au titre du bloc.
      if (!confirmed && outcome.ok && (!document.activeElement || document.activeElement === document.body)) {
        slotsHeading.current?.focus();
      }
    } else if (confirmed) {
      setRemoveError(errorText(outcome.code));
    } else {
      setSlotError(errorText(outcome.code));
    }
  }

  async function cancel() {
    const outcome = await actions.cancel();
    if (outcome.ok || outcome.handled) setConfirmCancel(false);
    else setCancelError(errorText(outcome.code));
  }

  const label = labelClass;
  const control = controlClass;
  const section = "border-t border-border pt-4";
  const heading = "text-label text-ink-secondary outline-none";
  const nameOf = (id: string) => {
    const c = reference?.classes.find((x) => x.id === id);
    return c ? labelOf(c.label, lang) : id;
  };
  const order = (id: string) => {
    const i = reference?.classes.findIndex((c) => c.id === id) ?? -1;
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  const boundRows = Object.keys(effective.bounds)
    .sort((a, b) => order(a) - order(b) || a.localeCompare(b))
    .map((id) => ({ id, name: nameOf(id), bound: effective.bounds[id] }));
  const instanceName = (id: string) => {
    const i = reference?.instances.find((x) => x.id === id);
    return i ? labelOf(i.label, lang) : id;
  };
  const regionName = (id: string) => {
    const r = reference?.regions.find((x) => x.id === id);
    return r ? labelOf(r.label, lang) : id;
  };
  const scopeName = (s: FactionScope) =>
    s === "all" ? t("settings.scope.all") : reference ? labelOf(reference.factions[s], lang) : s;
  const factions: FactionScope[] = ["all", "elyos", "asmodian"];
  const today = new Date(now());
  const slotName = (s: PublicSlot) => `${formatSlot(s.startsAt, lang)} ${formatDay(s.startsAt, lang, today)}`;

  return (
    <div data-testid="settings" className="mt-4 flex flex-col gap-6">
      <form noValidate onSubmit={save}>
        {/* Pendant l'envoi, la saisie est figée : la vue rendue remplacera le brouillon. */}
        <fieldset disabled={busy === "settings"} className="flex min-w-0 flex-col gap-5">
        <section data-testid="settings-raid" className="flex flex-col gap-4">
          <h2 className={heading}>{t("settings.raid")}</h2>
          {frozen ? (
            <div>
              <dl className="flex flex-col gap-1 text-body">
                <div className="flex flex-wrap gap-x-2">
                  <dt className="text-meta text-ink-secondary">{t("settings.instance")}</dt>
                  <dd>{instanceName(view.event.instance)}</dd>
                </div>
                <div className="flex flex-wrap gap-x-2">
                  <dt className="text-meta text-ink-secondary">{t("settings.region")}</dt>
                  <dd>{regionName(view.event.region)}</dd>
                </div>
                <div className="flex flex-wrap items-center gap-x-2">
                  <dt className="text-meta text-ink-secondary">{t("settings.scope")}</dt>
                  <dd className="inline-flex items-center gap-1">
                    {view.event.factionScope !== "all" && <FactionDot faction={view.event.factionScope} />}
                    {scopeName(view.event.factionScope)}
                  </dd>
                </div>
              </dl>
              <p className="mt-2 text-meta text-ink-secondary">{t("settings.frozen")}</p>
            </div>
          ) : (
            <>
              <div>
                <label htmlFor={`${ids}-instance`} className={label}>
                  {t("settings.instance")}
                </label>
                <select
                  id={`${ids}-instance`}
                  value={effective.instance}
                  onChange={(e) => setInstance(e.target.value)}
                  className={control}
                  {...errors.invalid("instance")}
                >
                  {missingInstance && <option value={missingInstance}>{missingInstance}</option>}
                  {instances.map((i) => (
                    <option key={i.id} value={i.id}>
                      {labelOf(i.label, lang)}
                    </option>
                  ))}
                </select>
                {errors.errorLine("instance")}
              </div>
              <div>
                <label htmlFor={`${ids}-region`} className={label}>
                  {t("settings.region")}
                </label>
                <select
                  id={`${ids}-region`}
                  value={effective.region}
                  onChange={(e) => {
                    errors.clearField("region");
                    patch({ region: e.target.value });
                  }}
                  className={control}
                  {...errors.invalid("region")}
                >
                  {missingRegion && <option value={missingRegion}>{missingRegion}</option>}
                  {regions.map((r) => (
                    <option key={r.id} value={r.id}>
                      {labelOf(r.label, lang)}
                    </option>
                  ))}
                </select>
                {errors.errorLine("region")}
              </div>
              <fieldset {...errors.invalid("factionScope")}>
                <legend className={label}>{t("settings.scope")}</legend>
                <div className="mt-1 grid grid-cols-3 gap-2">
                  {factions.map((f) => {
                    const disabled = f === "all" && !!instance && !instance.crossFaction;
                    return (
                      <label
                        key={f}
                        className={`flex min-h-tap items-center justify-center rounded-md border border-border px-2 text-center text-label has-checked:border-2 has-checked:border-accent has-focus-visible:outline-2 has-focus-visible:outline-accent ${
                          disabled ? "text-ink-disabled" : "text-ink-primary"
                        }`}
                      >
                        <input
                          type="radio"
                          name={`${ids}-scope`}
                          value={f}
                          checked={effective.factionScope === f}
                          disabled={disabled}
                          onChange={() => {
                            errors.clearField("factionScope");
                            patch({ factionScope: f });
                          }}
                          className="sr-only"
                        />
                        {scopeName(f)}
                      </label>
                    );
                  })}
                </div>
                {errors.errorLine("factionScope")}
              </fieldset>
            </>
          )}
        </section>

        <section data-testid="settings-details" className={`${section} flex flex-col gap-5`}>
          <h2 className={heading}>{t("settings.details")}</h2>
          <NoteField id={`${ids}-note`} value={draft.note} onChange={(note) => patch({ note })} errors={errors} />
          <DiscordField
            id={`${ids}-discord`}
            value={draft.discordUrl}
            onChange={(discordUrl) => patch({ discordUrl })}
            errors={errors}
          />
          <ListedField checked={draft.listed} onChange={(listed) => patch({ listed })} />
          <MinCpField
            id={`${ids}-mincp`}
            value={draft.minCp}
            onChange={(minCp, minCpBad) => patch({ minCp, minCpBad })}
            errors={errors}
          />
          <BoundsEditor
            rows={boundRows}
            cap={cap}
            errors={errors}
            onChange={(id, b) => setDraft((d) => ({ ...d, bounds: { ...d.bounds, [id]: b } }))}
          />
        </section>

        <div>
          <button
            type="submit"
            disabled={!!reason || busy !== null}
            aria-describedby={reason ? `${ids}-reason` : undefined}
            className="min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
          >
            {busy === "settings" ? t("settings.sending") : t("settings.save")}
          </button>
          {reason && (
            <p id={`${ids}-reason`} className="mt-2 text-meta text-ink-secondary">
              {t(reason, { max: NOTE_MAX })}
            </p>
          )}
          {saveError && (
            <p role="alert" className="mt-2 text-label text-danger">
              {saveError}
            </p>
          )}
        </div>
        </fieldset>
      </form>

      <section data-testid="settings-slots" className={section}>
        <h2 ref={slotsHeading} tabIndex={-1} className={heading}>
          {t("settings.slots")}
        </h2>
        {open ? (
          <>
            <p className="mt-1 text-meta text-ink-secondary">{t("event.timezone", { zone: timeZoneLabel(lang) })}</p>
            <ul className="mt-2 flex flex-col gap-1">
              {view.event.slots.map((s) => (
                <li
                  key={s.id}
                  data-testid={`slot-${s.id}`}
                  className={`flex min-h-tap items-center gap-2 text-label tabular-nums ${Date.parse(s.startsAt) <= now() ? "text-ink-disabled" : ""}`}
                >
                  <span>{formatSlot(s.startsAt, lang)}</span>
                  <span className="text-meta text-ink-secondary">{formatDay(s.startsAt, lang, today)}</span>
                  <span className="text-meta text-ink-secondary">
                    {s.durationMin} {t("editor.slot.minutes")}
                  </span>
                  {view.event.slots.length > 1 && (
                    <button
                      type="button"
                      aria-label={t("settings.slot.remove", { slot: slotName(s) })}
                      disabled={busy !== null}
                      onClick={(e) => askRemove(s, e.currentTarget)}
                      className="ml-auto flex size-tap shrink-0 items-center justify-center rounded-md border border-border text-ink-secondary disabled:text-ink-disabled"
                    >
                      ×
                    </button>
                  )}
                </li>
              ))}
            </ul>
            <div className="mt-4">
              <SlotInputs
                local={addLocal}
                duration={addDuration}
                instanceDuration={storedInstance?.durationMin}
                dateLabel={t("settings.slot.date")}
                durationLabel={t("settings.slot.duration")}
                onLocal={(v) => {
                  slotErrors.clearField("startsAt");
                  setAddLocal(v);
                }}
                onDuration={(v, bad) => {
                  slotErrors.clearField("durationMin");
                  setAddDuration(v);
                  setAddDurationBad(bad);
                }}
                dateAria={slotErrors.invalid("startsAt")}
                durationAria={slotErrors.invalid("durationMin")}
              />
              {slotErrors.errorLine("startsAt")}
              {slotErrors.errorLine("durationMin")}
              <button
                type="button"
                disabled={!canAdd}
                aria-describedby={addReason ? `${ids}-add-reason` : undefined}
                onClick={() => void add()}
                className="mt-3 min-h-tap rounded-md border border-border px-4 text-label text-ink-primary disabled:text-ink-disabled"
              >
                {busy === "addSlot" ? t("settings.sending") : t("settings.slot.add")}
              </button>
              {addReason && (
                <p id={`${ids}-add-reason`} className="mt-2 text-meta text-ink-secondary">
                  {t(addReason, { max: SLOTS_MAX })}
                </p>
              )}
              {slotError && (
                <p role="alert" className="mt-2 text-label text-danger">
                  {slotError}
                </p>
              )}
            </div>
          </>
        ) : (
          <p className="mt-1 text-body text-ink-secondary">{t("settings.locked")}</p>
        )}
      </section>

      <section className={section}>
        <button
          ref={cancelTrigger}
          type="button"
          disabled={busy !== null}
          onClick={() => {
            setCancelError(null);
            setConfirmCancel(true);
          }}
          className="min-h-tap w-full rounded-md border border-danger px-4 text-label text-danger disabled:border-border disabled:text-ink-disabled"
        >
          {t("settings.cancel")}
        </button>
      </section>

      {removing && (
        <ConfirmSheet
          message={t(removing.ticked === 1 ? "settings.slot.ticked.one" : "settings.slot.ticked.other", {
            count: removing.ticked,
          })}
          confirmLabel={t("settings.slot.delete")}
          cancelLabel={t("settings.slot.keep")}
          busyLabel={t("settings.sending")}
          busy={busy === "removeSlot"}
          error={removeError}
          onConfirm={() => void remove(removing.slot.id, true)}
          onClose={() => setRemoving(null)}
          trigger={removeTrigger}
          fallbackFocus={slotsHeading}
        />
      )}
      {confirmCancel && (
        <ConfirmSheet
          message={t("settings.cancelConfirm")}
          confirmLabel={t("settings.cancel")}
          cancelLabel={t("settings.keep")}
          busyLabel={t("settings.sending")}
          busy={busy === "cancel"}
          error={cancelError}
          onConfirm={() => void cancel()}
          onClose={() => setConfirmCancel(false)}
          trigger={cancelTrigger}
          fallbackFocus={fallbackFocus}
        />
      )}
    </div>
  );
}
