// Libellés de jeu lus dans la référence (libellé `fr`, sinon `en`), avec repli sur l'id quand la
// référence manque ou ne connaît pas l'entrée (AD-6). Partagés par les écrans et le message Discord.

import { labelOf } from "../api/reference";
import type { ReferenceSet } from "../api/types";

export function roleLabel(reference: ReferenceSet | null, role: string, lang: string): string {
  const r = reference?.roles.find((x) => x.id === role);
  return r ? labelOf(r.label, lang) : role;
}

export function instanceLabel(reference: ReferenceSet | null, instance: string, lang: string): string {
  const i = reference?.instances.find((x) => x.id === instance);
  return i ? labelOf(i.label, lang) : instance;
}
