// Message Discord de l'onglet Salle (SPEC epic 3 CAP-5, EXPERIENCE.md *Salle et message Discord*,
// `placement-et-salle.md` *Message Discord*). Fonction pure, sans React : un bloc par groupe retenu,
// dans l'ordre des créneaux, séparés par une ligne vide, puis une seule ligne pour tous, le lien
// public `/e/<id>` ; avec `groupId`, le bloc de ce seul groupe, puis le lien (un message trop long
// pour Discord se copie groupe par groupe). Chaque ligne vient d'un gabarit `discord.*` des locales,
// dans la langue donnée ; libellés d'instance et de rôle lus dans la référence (repli sur l'id,
// AD-6). L'heure part en horodatage Discord `<t:UNIX:F>`, que chaque lecteur voit dans son fuseau.
// Le secret du lien organisateur n'entre jamais ici : ni `/o/`, ni `/j/`, ni `/p/`.

import type { OrganizerEvent, ReferenceSet } from "../api/types";
import { translate, type Lang, type MessageKey, type Vars } from "../i18n";
import { instanceLabel, roleLabel } from "./labels";
import { retainedGroups } from "./placement";

/** Longueur maximale d'un message Discord, en caractères. */
export const DISCORD_MAX = 2000;

/** Horodatage Discord du début d'un créneau, en secondes UTC. */
export const discordTime = (startsAt: string) => `<t:${Math.floor(Date.parse(startsAt) / 1000)}:F>`;

/** Lien public de l'événement, composé avec l'origine du navigateur (l'URL publique en production). */
export const publicLink = (origin: string, eventId: string) => `${origin}/e/${eventId}`;

export function discordMessage(
  view: OrganizerEvent,
  reference: ReferenceSet | null,
  lang: Lang,
  origin: string,
  groupId?: string,
): string {
  const tr = (key: MessageKey, vars?: Vars) => translate(lang, key, vars);
  const instanceName = instanceLabel(reference, view.event.instance, lang);
  const byId = new Map(view.candidatures.map((c) => [c.id, c]));

  const groups = retainedGroups(view).filter(({ group }) => groupId === undefined || group.id === groupId);
  const blocks = groups.map(({ group }) => {
    const slot = view.event.slots.find((s) => s.id === group.slotId);
    const lines = [
      slot ? tr("discord.title", { instance: instanceName, time: discordTime(slot.startsAt) }) : instanceName,
      tr("discord.room", { room: group.roomName }),
    ];
    if (view.event.factionScope === "all") lines.push(tr("discord.faction"));
    for (let party = 1; party <= group.parties; party++) {
      lines.push(tr("discord.party", { n: party }));
      for (let index = 1; index <= group.partySize; index++) {
        const starter = group.starters.find((s) => s.party === party && s.index === index);
        const c = starter ? byId.get(starter.candidatureId) : undefined;
        lines.push(
          c
            ? tr("discord.slot", { n: index, player: c.pseudoSrv, role: roleLabel(reference, c.role, lang) })
            : tr("discord.free", { n: index }),
        );
      }
    }
    const bench = [...group.bench]
      .sort((a, b) => a.benchIndex - b.benchIndex)
      .map((b) => byId.get(b.candidatureId)?.pseudoSrv)
      .filter((p): p is string => !!p);
    if (bench.length > 0) lines.push(tr("discord.bench", { players: bench.join(", ") }));
    return lines.join("\n");
  });

  return [...blocks, publicLink(origin, view.event.id)].join("\n\n");
}
