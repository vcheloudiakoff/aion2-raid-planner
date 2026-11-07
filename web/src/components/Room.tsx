// Onglet Salle de `/o` (SPEC epic 3 CAP-4 et CAP-5, EXPERIENCE.md *Salle et message Discord*), chargé
// à la demande (`React.lazy`, chunk séparé, AD-19). En `locked` seulement.
// - En tête, « Copier le message Discord », le bouton principal : le message est construit ici par
//   `discordMessage` (pur, sans secret), copié, puis « Copié » 1,5 s ; si le presse-papiers échoue, il
//   s'affiche dans une zone en lecture seule, focalisée et entièrement sélectionnée à chaque échec,
//   et reconstruite depuis la vue courante tant qu'elle est ouverte. Au-delà de 2 000 caractères
//   (limite de Discord), une note dit de copier groupe par groupe. Un brouillon de composition non
//   enregistré est signalé : le message reprend le placement enregistré.
// - Une carte par groupe retenu, dans l'ordre des créneaux (« G1 · jeu. 21:00 ») : le nom de salle en
//   `display` à copier ; la recette en deux ou trois lignes (instance selon son genre, nom, « Faction :
//   All » si la portée est `all`) ; Équipe 1 puis Équipe 2, slot par slot (numéro, rôle du titulaire,
//   `Pseudo[SRV]` à copier, « pas dispo » s'il n'a pas coché le créneau, ou « libre ») ; le banc dans
//   l'ordre, absent s'il est vide ; « Copier le message de G1 », le bloc de ce seul groupe.
// - Menu « … » de la carte (Échap ou un toucher ailleurs le referme) : « Régénérer le nom », derrière
//   une confirmation qui prévient que le nom posté sur Discord deviendra faux. La commande passe par
//   la page (une seule à la fois : le bouton reste inactif tant qu'une autre est en vol) ; un refus
//   d'état y est traité (feuille fermée, vue relue), les autres erreurs restent dans la feuille.
// Le nom n'est jamais composé ici : il vient du serveur (AD-18).

import { useEffect, useId, useRef, useState, type RefObject } from "react";
import type { OrganizerCandidature, OrganizerEvent, ReferenceSet } from "../api/types";
import { t, useLang } from "../i18n";
import { DISCORD_MAX, discordMessage } from "../lib/discord";
import { instanceLabel, roleLabel } from "../lib/labels";
import { retainedGroups } from "../lib/placement";
import { formatSlot } from "../lib/time";
import { ConfirmSheet } from "./ConfirmSheet";
import { CopiedToast, CopyChip, TOAST_MS } from "./CopyChip";
import { SleepingGroups } from "./SleepingGroups";

/** Suite d'une demande de nouveau nom : fermer la confirmation, ou la garder avec une erreur
 * (`null` : rien à dire, par exemple une autre commande était en vol). */
export type RegenerateOutcome = { close: true } | { close: false; error: string | null };

export interface RoomProps {
  view: OrganizerEvent;
  reference: ReferenceSet | null;
  /** Origine du navigateur : le lien public du message en dérive. */
  origin: string;
  /** Nouveau nom pour ce groupe : la feuille se ferme sur une réussite ou un refus d'état traité
   * par la page, sinon elle reste ouverte avec l'erreur traduite. */
  onRegenerate: (groupId: string) => Promise<RegenerateOutcome>;
  /** La régénération est en vol. */
  regenerating: boolean;
  /** Une commande est en vol, de cet onglet ou d'un autre : aucun envoi en attendant. */
  busy: boolean;
  /** Un brouillon de composition non enregistré existe : le message ne le reprend pas. */
  draft: boolean;
  /** Repli du focus si le déclencheur a disparu. */
  fallbackFocus?: RefObject<HTMLElement | null>;
}

export function Room({ view, reference, origin, onRegenerate, regenerating, busy, draft, fallbackFocus }: RoomProps) {
  const lang = useLang();
  const ids = useId();
  const groups = retainedGroups(view);
  const byId = new Map(view.candidatures.map((c) => [c.id, c]));
  const slotOf = (id: string) => view.event.slots.find((s) => s.id === id);
  const kind = reference?.instances.find((i) => i.id === view.event.instance)?.kind;
  const instanceName = instanceLabel(reference, view.event.instance, lang);
  // Genre inconnu (référence absente ou instance qu'elle ne connaît pas) : une ligne neutre.
  const recipeKey =
    kind === "expedition"
      ? "room.recipe.expedition"
      : kind === "sanctuary"
        ? "room.recipe.sanctuary"
        : "room.recipe.unknown";
  const allFactions = view.event.factionScope === "all";

  const [toast, setToast] = useState(0);
  /** Repli ouvert : le message entier, ou celui d'un groupe. Le texte se reconstruit à chaque rendu,
   * depuis la vue courante. */
  const [fallback, setFallback] = useState<{ groupId?: string } | null>(null);
  /** Échecs du presse-papiers : chacun refocalise et resélectionne la zone. */
  const [failures, setFailures] = useState(0);
  const area = useRef<HTMLTextAreaElement>(null);
  /** Carte dont le menu « … » est ouvert : un toucher ailleurs le referme. */
  const menuWrap = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{ groupId: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const trigger = useRef<HTMLElement | null>(null);
  /** Boutons « … » par groupe : le focus y revient à la fermeture de la confirmation. */
  const moreButtons = useRef<Record<string, HTMLButtonElement | null>>({});
  const heading = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(0), TOAST_MS);
    return () => clearTimeout(id);
  }, [toast]);

  useEffect(() => {
    // Repli du presse-papiers : tout le message, sélectionné, prêt pour une copie à la main.
    if (!failures) return;
    area.current?.focus();
    area.current?.select();
  }, [failures]);

  // Échap referme le menu « … » ouvert et rend le focus à son bouton ; un toucher hors de la carte
  // le referme aussi.
  useEffect(() => {
    if (!menu) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      setMenu(null);
      moreButtons.current[menu]?.focus();
    };
    const onClick = (e: MouseEvent) => {
      if (!menuWrap.current?.contains(e.target as Node)) setMenu(null);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", onClick);
    };
  }, [menu]);

  // Groupe sorti des groupes retenus pendant la confirmation : plus rien à renommer.
  const confirmShown = confirm !== null && groups.some((g) => g.group.id === confirm.groupId);
  useEffect(() => {
    if (confirm && !confirmShown) setConfirm(null);
  }, [confirm, confirmShown]);

  const fullMessage = discordMessage(view, reference, lang, origin);
  const fallbackGone = fallback?.groupId !== undefined && !groups.some((g) => g.group.id === fallback.groupId);
  const fallbackText =
    fallback === null || fallbackGone
      ? null
      : fallback.groupId === undefined
        ? fullMessage
        : discordMessage(view, reference, lang, origin, fallback.groupId);

  /** Copie le message entier, ou celui d'un groupe. */
  async function copyMessage(groupId?: string) {
    const text = groupId === undefined ? fullMessage : discordMessage(view, reference, lang, origin, groupId);
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      setToast(0);
      setFallback({ groupId });
      setFailures((n) => n + 1);
      return;
    }
    setFallback(null);
    // Un nouveau toucher relance les 1,5 s.
    setToast((n) => n + 1);
  }

  async function regenerate() {
    if (!confirm || busy) return;
    setError(null);
    const outcome = await onRegenerate(confirm.groupId);
    if (outcome.close) setConfirm(null);
    else setError(outcome.error);
  }

  const person = (c: OrganizerCandidature | undefined, unavailable: boolean) =>
    c ? { pseudoSrv: c.pseudoSrv, role: roleLabel(reference, c.role, lang), unavailable } : null;
  const unavailableMark = (
    <span data-testid="room-unavailable" className="block text-meta text-danger">
      {t("composition.unavailable")}
    </span>
  );

  return (
    <section data-testid="room" className="mt-4">
      <h2 ref={heading} tabIndex={-1} className="sr-only">
        {t("organizer.tab.room")}
      </h2>
      {groups.length === 0 ? (
        <p className="text-body text-ink-secondary">{t("groups.empty")}</p>
      ) : (
        <>
          {draft && (
            <p role="status" data-testid="room-draft" className="mb-3 text-label text-ink-secondary">
              {t("room.draft")}
            </p>
          )}
          <button
            type="button"
            data-testid="room-copy-discord"
            onClick={() => void copyMessage()}
            className="min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent"
          >
            {t("room.copyDiscord")}
          </button>
          {fullMessage.length > DISCORD_MAX && (
            <p data-testid="room-too-long" className="mt-2 text-meta text-ink-secondary">
              {t("room.tooLong", { max: DISCORD_MAX.toLocaleString(lang) })}
            </p>
          )}
          {fallbackText !== null && (
            <div className="mt-3">
              <p id={`${ids}-fallback`} role="status" className="text-meta text-ink-secondary">
                {t("room.fallback")}
              </p>
              <textarea
                ref={area}
                data-testid="room-message"
                readOnly
                value={fallbackText}
                aria-label={t("room.message")}
                aria-describedby={`${ids}-fallback`}
                rows={12}
                className="mt-2 w-full rounded-sm border border-border bg-surface-sunken p-3 font-mono text-[15px] leading-[1.4] text-ink-primary"
              />
            </div>
          )}
        </>
      )}

      {groups.map(({ n, group }) => {
        const slot = slotOf(group.slotId);
        const menuId = `${ids}-menu-${group.id}`;
        const parties = Array.from({ length: group.parties }, (_, i) => i + 1);
        const indexes = Array.from({ length: group.partySize }, (_, i) => i + 1);
        const bench = [...group.bench]
          .sort((a, b) => a.benchIndex - b.benchIndex)
          .flatMap((b) => {
            const c = byId.get(b.candidatureId);
            return c ? [{ c, unavailable: b.unavailable }] : [];
          });
        const recipe = [
          t(recipeKey, { instance: instanceName }),
          t("room.recipe.name", { room: group.roomName }),
          ...(allFactions ? [t("room.recipe.faction")] : []),
        ];
        return (
          <article
            key={group.id}
            data-testid={`room-card-${group.id}`}
            aria-labelledby={`${ids}-card-${group.id}`}
            className="mt-4 rounded-md border border-border bg-surface-raised p-3"
          >
            <div ref={menu === group.id ? menuWrap : undefined}>
              <div className="flex items-center gap-2">
                <h3 id={`${ids}-card-${group.id}`} className="min-w-0 flex-1 text-label tabular-nums">
                  {t("room.card", { n, slot: slot ? formatSlot(slot.startsAt, lang) : "" })}
                </h3>
                <button
                  ref={(el) => {
                    moreButtons.current[group.id] = el;
                  }}
                  type="button"
                  data-testid={`room-more-${group.id}`}
                  aria-label={t("room.more", { n })}
                  aria-expanded={menu === group.id}
                  aria-controls={menu === group.id ? menuId : undefined}
                  onClick={() => setMenu((m) => (m === group.id ? null : group.id))}
                  className="min-h-tap min-w-tap shrink-0 rounded-md border border-border px-3 text-label text-ink-primary"
                >
                  …
                </button>
              </div>
              {menu === group.id && (
                <div id={menuId} className="mt-2">
                  <button
                    type="button"
                    data-testid={`room-regenerate-${group.id}`}
                    disabled={busy}
                    onClick={() => {
                      // Le bouton disparaît avec le menu : le focus reviendra sur « … ».
                      trigger.current = moreButtons.current[group.id] ?? null;
                      setMenu(null);
                      setError(null);
                      setConfirm({ groupId: group.id });
                    }}
                    className="min-h-tap w-full rounded-md border border-danger px-4 text-left text-label text-danger disabled:border-border disabled:text-ink-disabled"
                  >
                    {t("room.regenerate")}
                  </button>
                </div>
              )}
            </div>

            <div className="mt-3">
              <CopyChip value={group.roomName} label={t("room.name")} variant="display" />
            </div>

            <h4 className="sr-only">{t("room.recipe")}</h4>
            <ol data-testid="room-recipe" className="mt-3 list-decimal pl-5 text-body [overflow-wrap:anywhere]">
              {recipe.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ol>

            {parties.map((party) => (
              <section key={party} className="mt-4">
                <h4 className="text-label">{t("composition.party", { n: party })}</h4>
                <ol className="mt-1 flex flex-col gap-1">
                  {indexes.map((index) => {
                    const starter = group.starters.find((s) => s.party === party && s.index === index);
                    const p = person(starter ? byId.get(starter.candidatureId) : undefined, !!starter?.unavailable);
                    return (
                      <li
                        key={index}
                        data-testid={`room-slot-${group.id}-${party}-${index}`}
                        className="flex min-h-[52px] items-center gap-3"
                      >
                        <span className="w-5 shrink-0 text-label tabular-nums text-ink-secondary">{index}</span>
                        {p ? (
                          <span className="min-w-0 flex-1">
                            <span className="block text-meta text-ink-secondary">{p.role}</span>
                            <CopyChip
                              value={p.pseudoSrv}
                              label={t("composition.slot.label", { party, index, who: p.pseudoSrv })}
                            />
                            {p.unavailable && unavailableMark}
                          </span>
                        ) : (
                          <span className="min-w-0 flex-1 text-label text-ink-disabled">{t("composition.free")}</span>
                        )}
                      </li>
                    );
                  })}
                </ol>
              </section>
            ))}

            {bench.length > 0 && (
              <section data-testid={`room-bench-${group.id}`} className="mt-4">
                <h4 className="text-label">{t("composition.bench")}</h4>
                <ol className="mt-1 flex flex-col gap-1">
                  {bench.map(({ c, unavailable }, i) => (
                    <li key={c.id} className="flex items-center gap-3">
                      <span className="w-5 shrink-0 text-label tabular-nums text-ink-secondary">{i + 1}</span>
                      <span className="min-w-0 flex-1">
                        <CopyChip
                          value={c.pseudoSrv}
                          label={t("composition.bench.label", { n: i + 1, who: c.pseudoSrv })}
                        />
                        {unavailable && unavailableMark}
                      </span>
                    </li>
                  ))}
                </ol>
              </section>
            )}

            <button
              type="button"
              data-testid={`room-copy-group-${group.id}`}
              onClick={() => void copyMessage(group.id)}
              className="mt-4 min-h-tap w-full rounded-md border border-border px-4 text-label text-ink-primary"
            >
              {t("room.copyGroup", { n })}
            </button>
          </article>
        );
      })}

      <SleepingGroups view={view} testId="room" />

      {toast > 0 && <CopiedToast />}

      {confirm && confirmShown && (
        <ConfirmSheet
          message={t("room.regenerateConfirm")}
          confirmLabel={t("room.regenerateAction")}
          cancelLabel={t("room.keep")}
          busyLabel={t("composition.sending")}
          busy={regenerating}
          confirmDisabled={busy}
          error={error}
          onConfirm={() => void regenerate()}
          onClose={() => setConfirm(null)}
          trigger={trigger}
          fallbackFocus={fallbackFocus ?? heading}
        />
      )}
    </section>
  );
}

export default Room;
