// Champs de l'événement partagés par la création (`/new`) et les réglages (`/o`) : note avec son
// compteur, lien Discord, « Lister publiquement » et CP minimum. Chaque champ efface son erreur du
// serveur dès qu'il change ; les libellés viennent des locales (`editor.*`).

import type { FieldErrors } from "../lib/form";
import { NOTE_MAX, chars } from "../lib/form";
import { t, useLang } from "../i18n";

export const labelClass = "block text-label";
export const controlClass =
  "mt-1 block min-h-tap w-full min-w-0 rounded-md border border-border bg-surface-sunken px-3 text-body text-ink-primary aria-[invalid=true]:border-danger";

interface TextProps {
  id: string;
  value: string;
  onChange: (value: string) => void;
  errors: FieldErrors;
}

export function NoteField({ id, value, onChange, errors }: TextProps) {
  useLang();
  return (
    <div>
      <div className="flex items-baseline justify-between gap-2">
        <label htmlFor={id} className={labelClass}>
          {t("editor.note")}
        </label>
        <span className={`text-meta tabular-nums ${chars(value) > NOTE_MAX ? "text-danger" : "text-ink-secondary"}`}>
          {t("editor.noteCount", { count: chars(value), max: NOTE_MAX })}
        </span>
      </div>
      <textarea
        id={id}
        rows={3}
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          errors.clearField("note");
        }}
        className={`${controlClass} py-2`}
        {...errors.invalid("note")}
      />
      {errors.errorLine("note")}
    </div>
  );
}

export function DiscordField({ id, value, onChange, errors }: TextProps) {
  useLang();
  return (
    <div>
      <label htmlFor={id} className={labelClass}>
        {t("editor.discord")}
      </label>
      <input
        id={id}
        type="url"
        inputMode="url"
        autoComplete="off"
        placeholder="https://discord.gg/…"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          errors.clearField("discordUrl");
        }}
        className={controlClass}
        {...errors.invalid("discordUrl")}
      />
      {errors.errorLine("discordUrl")}
    </div>
  );
}

interface NumberProps extends Omit<TextProps, "onChange"> {
  /** `bad` : le navigateur ne sait pas lire la saisie (`validity.badInput`) et rapporte une valeur
   * vide ; ce n'est pas un champ effacé. */
  onChange: (value: string, bad: boolean) => void;
}

/** Valeur d'un champ numérique et saisie illisible. Lu aussi sur `input` : React ne signale pas un
 * changement quand la valeur rapportée reste vide, d'une saisie illisible à une autre ou à rien. */
export function numberInput(onChange: (value: string, bad: boolean) => void) {
  const report = (el: HTMLInputElement) => onChange(el.value, el.validity?.badInput ?? false);
  return {
    onChange: (e: { target: HTMLInputElement }) => report(e.target),
    onInput: (e: { currentTarget: HTMLInputElement }) => report(e.currentTarget),
  };
}

export function MinCpField({ id, value, onChange, errors }: NumberProps) {
  useLang();
  return (
    <div>
      <label htmlFor={id} className={labelClass}>
        {t("editor.minCp")}
      </label>
      <input
        id={id}
        type="number"
        inputMode="numeric"
        min={0}
        max={999999}
        value={value}
        {...numberInput((v, bad) => {
          onChange(v, bad);
          errors.clearField("minCp");
        })}
        className={`${controlClass} tabular-nums`}
        {...errors.invalid("minCp")}
      />
      {errors.errorLine("minCp")}
    </div>
  );
}

export function ListedField({ checked, onChange }: { checked: boolean; onChange: (checked: boolean) => void }) {
  useLang();
  return (
    <label className="flex min-h-tap items-center gap-3 text-body">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-5 accent-[var(--accent)]"
      />
      {t("editor.listed")}
    </label>
  );
}
