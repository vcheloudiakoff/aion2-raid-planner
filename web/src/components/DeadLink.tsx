// Lien mort (EXPERIENCE.md *State Patterns*), partagé par `/e`, `/j`, `/o` et `/p` : un seul message
// pour un lien inconnu, expiré ou d'un autre droit, puis « Retour à l'accueil ». Seul un lien qui ne
// mène plus à rien (`isDeadLinkCode`) retire son entrée de « Sur cet appareil » ; un lien d'un autre
// droit, une erreur réseau, un quota ou une panne ne retirent jamais rien.

import { Link } from "react-router";
import { t, useLang } from "../i18n";

/** Codes terminaux : un seul message, « ce lien ne mène à rien », et plus aucune relecture. */
export const GONE = new Set(["event.not_found", "auth.not_found", "auth.wrong_right", "auth.missing"]);

/** Vrai si le lien ne mène plus à rien : son entrée locale est à oublier. */
export function isDeadLinkCode(code: string | undefined): boolean {
  return code === "auth.not_found" || code === "event.not_found";
}

/** Lien inconnu, expiré ou d'un autre droit : un seul message, puis le retour à l'accueil. */
export function DeadLink() {
  useLang();
  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <p role="alert" className="text-body">
        {t("event.notFound")}
      </p>
      <Link
        to="/"
        className="mt-6 inline-flex min-h-tap items-center rounded-md border border-border px-4 text-label text-ink-primary no-underline"
      >
        {t("event.home")}
      </Link>
    </main>
  );
}
