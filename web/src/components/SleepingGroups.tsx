// Compte des groupes en sommeil (SPEC epic 3 CAP-2) : un groupe non repris au re-verrouillage n'a ni
// numéro ni carte, seulement cette ligne sous les groupes retenus (il compte aussi dans la
// confirmation de suppression d'un créneau). Rien si aucun groupe ne dort. La numérotation « G1 »
// des groupes retenus est `retainedGroups` (`lib/placement`).

import type { OrganizerEvent } from "../api/types";
import { t, useLang } from "../i18n";

interface Props {
  view: OrganizerEvent;
  /** Préfixe du `data-testid` : la ligne porte `${testId}-sleeping`. */
  testId: string;
}

export function SleepingGroups({ view, testId }: Props) {
  useLang();
  const sleeping = view.groups.filter((g) => !g.retained).length;
  if (sleeping === 0) return null;
  return (
    <p data-testid={`${testId}-sleeping`} className="mt-4 text-meta text-ink-secondary">
      {t(sleeping === 1 ? "groups.sleeping.one" : "groups.sleeping.other", { count: sleeping })}
    </p>
  );
}
