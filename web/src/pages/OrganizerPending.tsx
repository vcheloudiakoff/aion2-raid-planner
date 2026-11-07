// `/o/<secret>` en attente de la vue organisateur (story 2-4). Aucune requête : le secret n'est ni
// lu, ni envoyé, ni affiché.

import { Link } from "react-router";
import { t, useLang } from "../i18n";

export function OrganizerPending() {
  useLang();
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 className="text-title">{t("organizer.pending.title")}</h1>
      <p className="mt-4 text-ink-secondary">{t("organizer.pending.body")}</p>
      <Link
        to="/"
        className="mt-6 inline-flex min-h-tap items-center rounded-md border border-border px-4 text-label text-ink-primary no-underline"
      >
        {t("organizer.pending.home")}
      </Link>
    </main>
  );
}
