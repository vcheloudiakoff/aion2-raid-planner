// Accueil (CAP-10) : état vide sans faux contenu et bouton « Créer un raid » (2-1). La liste
// publique et « Sur cet appareil » arrivent avec leurs stories.

import { Link } from "react-router";
import { t, useLang } from "../i18n";

export function Home() {
  useLang();
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 className="text-title">{t("home.title")}</h1>
      <p className="mt-4 text-ink-secondary">{t("home.empty")}</p>
      <Link
        to="/new"
        className="mt-6 flex min-h-tap w-full items-center justify-center rounded-md bg-accent px-4 text-label text-on-accent no-underline"
      >
        {t("home.create")}
      </Link>
    </main>
  );
}
