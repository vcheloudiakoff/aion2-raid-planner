// Accueil (CAP-10, CAP-6) : état vide sans faux contenu, bouton « Créer un raid » (2-1), puis « Sur cet
// appareil », masqué quand ce navigateur ne garde aucun raid ni aucune candidature (2-6). La liste
// publique arrive avec l'epic 5.

import { useRef } from "react";
import { Link } from "react-router";
import type { ReferenceSet } from "../api/types";
import { OnThisDevice } from "../components/OnThisDevice";
import { t, useLang } from "../i18n";

interface Props {
  reference: ReferenceSet | null;
  now?: () => number;
}

export function Home({ reference, now }: Props) {
  useLang();
  const heading = useRef<HTMLHeadingElement>(null);
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 ref={heading} tabIndex={-1} className="text-title outline-none">
        {t("home.title")}
      </h1>
      <p className="mt-4 text-ink-secondary">{t("home.empty")}</p>
      <Link
        to="/new"
        className="mt-6 flex min-h-tap w-full items-center justify-center rounded-md bg-accent px-4 text-label text-on-accent no-underline"
      >
        {t("home.create")}
      </Link>
      <OnThisDevice reference={reference} fallbackFocus={heading} now={now} />
    </main>
  );
}
