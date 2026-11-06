// Bandeaux d'état globaux (EXPERIENCE.md, State Patterns) : hors ligne et nouvelle version. Discrets,
// sans rechargement forcé : un formulaire en cours n'est jamais perdu.

import { useClientStatus } from "../api/client";
import { t, useLang } from "../i18n";

interface Props {
  /** Heure de la dernière donnée confirmée par le serveur, ISO 8601 UTC. */
  lastDataAt: string | null;
  reload?: () => void;
}

export function formatTime(iso: string, lang: string): string {
  return new Intl.DateTimeFormat(lang, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(
    new Date(iso),
  );
}

export function Banners({ lastDataAt, reload = () => window.location.reload() }: Props) {
  const lang = useLang();
  const { offline, newBuild } = useClientStatus();
  return (
    <div role="status" className="mx-auto max-w-content px-3 empty:hidden min-[360px]:px-4">
      {offline && (
        <p className="mt-3 rounded-md border border-border bg-surface-sunken px-3 py-2 text-meta text-ink-secondary">
          {lastDataAt ? t("banner.offlineSince", { time: formatTime(lastDataAt, lang) }) : t("banner.offline")}
        </p>
      )}
      {newBuild && (
        <div className="mt-3 flex items-center justify-between gap-3 rounded-md border border-border bg-surface-raised py-1 pr-1 pl-3">
          <span className="text-meta text-ink-primary">{t("banner.newVersion")}</span>
          <button
            type="button"
            onClick={reload}
            className="min-h-tap rounded-md border border-border px-4 text-label text-ink-primary"
          >
            {t("banner.reload")}
          </button>
        </div>
      )}
    </div>
  );
}
