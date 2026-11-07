// Saisie d'un créneau (EXPERIENCE.md *Formulaires*), partagée par la création et les réglages : date
// et heure natives en heure locale, puis la durée en minutes, l'indice étant la durée de l'instance.
// La conversion en UTC reste à l'appelant (AD-11) ; `children` reçoit le bouton de fin de ligne.

import type { ReactNode } from "react";
import { t, useLang } from "../i18n";
import { DURATION_MAX, DURATION_MIN } from "../lib/form";
import { controlClass, numberInput } from "./EventFields";

type Aria = { "aria-invalid"?: true; "aria-describedby"?: string };

interface Props {
  /** Valeur de `datetime-local`, heure locale. */
  local: string;
  /** Vide = durée de l'instance. */
  duration: string;
  /** Durée de l'instance, en indice ; absente tant que la référence n'est pas là. */
  instanceDuration?: number;
  dateLabel: string;
  durationLabel: string;
  onLocal: (value: string) => void;
  /** `bad` : saisie que le navigateur ne sait pas lire (`validity.badInput`), rapportée vide. */
  onDuration: (value: string, bad: boolean) => void;
  dateAria?: Aria;
  durationAria?: Aria;
  children?: ReactNode;
}

export function SlotInputs({
  local,
  duration,
  instanceDuration,
  dateLabel,
  durationLabel,
  onLocal,
  onDuration,
  dateAria,
  durationAria,
  children,
}: Props) {
  useLang();
  // `controlClass` porte `w-full` : dans une ligne, il écraserait la largeur fixe de la durée et
  // réduirait le champ date à rien. La ligne gère elle-même les largeurs.
  const rowControl = controlClass.replace(" w-full", "");
  return (
    <div className="flex items-center gap-2">
      <input
        type="datetime-local"
        aria-label={dateLabel}
        value={local}
        onChange={(e) => onLocal(e.target.value)}
        className={`${rowControl} mt-0 flex-1 px-2`}
        {...dateAria}
      />
      <input
        type="number"
        inputMode="numeric"
        min={DURATION_MIN}
        max={DURATION_MAX}
        aria-label={durationLabel}
        value={duration}
        placeholder={instanceDuration !== undefined ? String(instanceDuration) : ""}
        {...numberInput(onDuration)}
        className={`${rowControl} mt-0 w-16 shrink-0 px-2 text-center tabular-nums`}
        {...durationAria}
      />
      <span className="text-meta text-ink-secondary">{t("editor.slot.minutes")}</span>
      {children}
    </div>
  );
}
