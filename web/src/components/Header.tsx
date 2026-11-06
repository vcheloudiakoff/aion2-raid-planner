// En-tête global : nom de l'outil à gauche, bouton « Langue · Thème » à droite. Pas de hamburger.

import { useRef, useState } from "react";
import { Link } from "react-router";
import { t, useLang } from "../i18n";
import { SettingsSheet } from "./SettingsSheet";

export function Header() {
  useLang();
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  return (
    <header className="border-b border-border bg-surface-base">
      <div className="mx-auto flex max-w-content items-center justify-between gap-2 px-3 py-1 min-[360px]:px-4">
        <Link to="/" className="flex min-h-tap min-w-0 items-center text-label text-ink-primary no-underline">
          <span className="truncate">{t("app.name")}</span>
        </Link>
        <button
          ref={trigger}
          type="button"
          aria-haspopup="dialog"
          aria-expanded={open}
          onClick={() => setOpen(true)}
          className="min-h-tap shrink-0 rounded-md border border-border px-3 text-label text-ink-primary"
        >
          {t("header.settings")}
        </button>
      </div>
      {open && <SettingsSheet trigger={trigger} onClose={() => setOpen(false)} />}
    </header>
  );
}
