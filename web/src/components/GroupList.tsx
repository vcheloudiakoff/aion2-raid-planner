// Liste des groupes retenus (SPEC epic 3 CAP-2) : « G1 · jeu. 21:00 · LUDRA-K7Q2 », dans l'ordre
// reçu (début de créneau puis occurrence). Le numéro « G1 » se calcule ici, sur les seuls groupes
// `retained` : un groupe en sommeil n'a ni numéro ni ligne, seulement un compte sous la liste (il
// compte aussi dans la confirmation de suppression d'un créneau). Contenu minimal des onglets Composition
// et Salle en story 3-2, sans action ; le placement (3-3) et la salle (3-4) le remplacent.

import type { OrganizerEvent, OrganizerGroup } from "../api/types";
import { t, useLang } from "../i18n";
import { formatSlot } from "../lib/time";

export interface NumberedGroup {
  /** 1 pour « G1 ». */
  n: number;
  group: OrganizerGroup;
}

/** Groupes retenus, numérotés dans l'ordre reçu. */
export function retainedGroups(view: OrganizerEvent): NumberedGroup[] {
  return view.groups.filter((g) => g.retained).map((group, i) => ({ n: i + 1, group }));
}

interface Props {
  view: OrganizerEvent;
  /** Titre du panneau, lu par le lecteur d'écran. */
  title: string;
  testId: string;
}

export function GroupList({ view, title, testId }: Props) {
  const lang = useLang();
  const groups = retainedGroups(view);
  const sleeping = view.groups.length - groups.length;
  const slotOf = (id: string) => view.event.slots.find((s) => s.id === id);
  return (
    <section data-testid={testId} className="mt-4">
      <h2 className="sr-only">{title}</h2>
      {groups.length === 0 ? (
        <p className="text-body text-ink-secondary">{t("groups.empty")}</p>
      ) : (
        <ul className="flex flex-col">
          {groups.map(({ n, group }) => {
            const slot = slotOf(group.slotId);
            return (
              <li
                key={group.id}
                data-testid={`group-${group.id}`}
                className="flex min-h-tap items-center border-b border-border py-2 text-label tabular-nums [overflow-wrap:anywhere]"
              >
                {t("groups.item", {
                  n,
                  slot: slot ? formatSlot(slot.startsAt, lang) : "",
                  room: group.roomName,
                })}
              </li>
            );
          })}
        </ul>
      )}
      {sleeping > 0 && (
        <p data-testid={`${testId}-sleeping`} className="mt-3 text-meta text-ink-secondary">
          {t(sleeping === 1 ? "groups.sleeping.one" : "groups.sleeping.other", { count: sleeping })}
        </p>
      )}
    </section>
  );
}
