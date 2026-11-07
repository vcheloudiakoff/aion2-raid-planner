// Résumé public d'un événement (CAP-2, EXPERIENCE.md *Formulaires*) : instance et date en titre,
// région, portée, note, créneaux en heure locale avec le fuseau une fois, ligne de rôles, CP minimum.
// Partagé par la page de l'événement et par Ma candidature. Libellés de jeu lus dans la référence,
// avec repli sur l'id (AD-6).

import { labelOf } from "../api/reference";
import type { Faction, PublicEvent, ReferenceSet } from "../api/types";
import { t, useLang } from "../i18n";
import { formatDay, formatSlot, timeZoneLabel } from "../lib/time";

export function FactionDot({ faction }: { faction: Faction }) {
  return (
    <span
      aria-hidden="true"
      className={`inline-block size-2 shrink-0 rounded-full ${faction === "elyos" ? "bg-faction-elyos" : "bg-faction-asmodian"}`}
    />
  );
}

export function roleLabel(reference: ReferenceSet | null, role: string, lang: string): string {
  const r = reference?.roles.find((x) => x.id === role);
  return r ? labelOf(r.label, lang) : role;
}

export interface SummaryProps {
  event: PublicEvent;
  reference: ReferenceSet | null;
  now: () => number;
  /** Niveau du titre : `h1` sur la page de l'événement, `h2` sous un autre titre de page. */
  heading?: "h1" | "h2";
}

export function Summary({ event, reference, now, heading: Heading = "h1" }: SummaryProps) {
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
      <Heading className="text-title">{first ? `${name} · ${formatDay(first.startsAt, lang, today)}` : name}</Heading>
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
