// Étape « Liens créés » de `/new` (EXPERIENCE.md, *Liens et appareils*) : trois puces dans l'ordre
// organisateur, direct, public. « Ouvrir mon raid » reste désactivé tant que le lien d'organisateur
// n'est ni copié, ni partagé, ni déclaré gardé. Le secret n'apparaît que dans les puces : jamais dans
// `document.title` ni dans les données de partage autres que l'URL.

import { useEffect, useId, useRef, useState } from "react";
import { useNavigate } from "react-router";
import type { CreatedEvent } from "../api/types";
import { CopyChip } from "../components/CopyChip";
import { t, useLang } from "../i18n";

interface Props {
  created: CreatedEvent;
  /** Faux si `saveOrganizerLinks` n'a pas pu écrire : ce navigateur ne gardera pas le lien. */
  stored: boolean;
  origin?: string;
}

export function LinksCreated({ created, stored, origin = window.location.origin }: Props) {
  useLang();
  const navigate = useNavigate();
  const [kept, setKept] = useState(false);
  const [checked, setChecked] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const reasonId = useId();
  const organizerUrl = origin + created.links.organizer;
  const canShare = typeof navigator.share === "function";
  const open = kept || checked;

  useEffect(() => {
    heading.current?.focus();
  }, []);

  async function share() {
    try {
      await navigator.share({ url: organizerUrl });
      setKept(true);
    } catch {
      // Partage annulé ou refusé : rien n'est gardé.
    }
  }

  return (
    <main className="mx-auto max-w-content px-3 py-6 min-[360px]:px-4">
      <h1 ref={heading} tabIndex={-1} className="text-title outline-none">
        {t("links.title")}
      </h1>
      {!stored && (
        <p role="alert" className="mt-4 rounded-md border border-danger px-3 py-2 text-label text-danger">
          {t("links.noStorage")}
        </p>
      )}

      <section data-testid="link-organizer" className="mt-5">
        <h2 className="text-label">{t("links.organizer")}</h2>
        <p className="mt-1 text-meta text-ink-secondary">{t("links.organizerWarning")}</p>
        <div className="mt-2">
          <CopyChip value={organizerUrl} label={t("links.organizer")} onCopied={() => setKept(true)} />
        </div>
        {canShare && (
          <button
            type="button"
            onClick={share}
            className="mt-2 min-h-tap rounded-md border border-border px-4 text-label text-ink-primary"
          >
            {t("links.share")}
          </button>
        )}
      </section>

      <section data-testid="link-join" className="mt-5">
        <h2 className="text-label">{t("links.join")}</h2>
        <p className="mt-1 text-meta text-ink-secondary">{t("links.joinHint")}</p>
        <div className="mt-2">
          <CopyChip value={origin + created.links.join} label={t("links.join")} />
        </div>
      </section>

      <section data-testid="link-public" className="mt-5">
        <h2 className="text-label">{t("links.public")}</h2>
        <p className="mt-1 text-meta text-ink-secondary">{t("links.publicHint")}</p>
        <div className="mt-2">
          <CopyChip value={origin + created.links.public} label={t("links.public")} />
        </div>
      </section>

      <label className="mt-6 flex min-h-tap items-center gap-3 text-body">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => setChecked(e.target.checked)}
          className="size-5 accent-[var(--accent)]"
        />
        {t("links.kept")}
      </label>

      <button
        type="button"
        disabled={!open}
        aria-describedby={open ? undefined : reasonId}
        onClick={() => navigate(created.links.organizer)}
        className="mt-4 min-h-tap w-full rounded-md bg-accent px-4 text-label text-on-accent disabled:bg-surface-sunken disabled:text-ink-disabled"
      >
        {t("links.open")}
      </button>
      {!open && (
        <p id={reasonId} className="mt-2 text-meta text-ink-secondary">
          {t("links.openReason")}
        </p>
      )}
    </main>
  );
}
