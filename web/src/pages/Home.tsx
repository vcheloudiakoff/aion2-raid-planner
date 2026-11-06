// Accueil vide (CAP-10) : pas de faux contenu, pas de bouton « Créer un raid » avant l'epic 2, pas
// de section « Sur cet appareil » avant la story 1-6.

import { t, useLang } from "../i18n";

export function Home() {
  useLang();
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 className="text-title">{t("home.title")}</h1>
      <p className="mt-4 text-ink-secondary">{t("home.empty")}</p>
    </main>
  );
}
