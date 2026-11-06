// Contrôle de CI (AD-5, AD-12) : chaque code du registre Rust a son texte dans chaque locale, et
// chaque clé `errors.*` correspond à un code (une traduction orpheline masque une faute de frappe).
//   cargo run -q -p server -- list-error-codes | node web/scripts/check-error-codes.mjs [LOCALES_DIR]
// Codes sur stdin, un par ligne. Code 1 et problèmes nommés sur stderr en cas d'écart.

import { readFileSync, readdirSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PREFIX = "errors.";

/** Codes non vides, sans doublon, dans l'ordre reçu. */
export function parseCodes(text) {
  return [...new Set(text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean))];
}

/** Problèmes entre les codes et des locales `{ nom: { clé: texte } }`, vide si tout concorde. */
export function check(codes, locales) {
  const problems = [];
  if (codes.length === 0) problems.push("no error code on stdin");
  const known = new Set(codes);
  for (const [name, messages] of Object.entries(locales)) {
    for (const code of codes) {
      const text = messages[PREFIX + code];
      if (typeof text !== "string" || text.trim() === "") problems.push(`${name}: missing ${PREFIX}${code}`);
    }
    for (const key of Object.keys(messages)) {
      if (key.startsWith(PREFIX) && !known.has(key.slice(PREFIX.length))) {
        problems.push(`${name}: ${key} has no error code`);
      }
    }
  }
  return problems;
}

/** Toutes les locales `*.json` d'un dossier. */
export function readLocales(dir) {
  const locales = {};
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json")).sort()) {
    locales[file] = JSON.parse(readFileSync(join(dir, file), "utf8"));
  }
  if (Object.keys(locales).length === 0) throw new Error(`no locale in ${dir}`);
  return locales;
}

function main() {
  const dir = resolve(process.argv[2] ?? join(fileURLToPath(new URL(".", import.meta.url)), "../src/locales"));
  const codes = parseCodes(readFileSync(0, "utf8"));
  const locales = readLocales(dir);
  const problems = check(codes, locales);
  for (const p of problems) console.error(p);
  if (problems.length > 0) process.exit(1);
  console.log(`error codes OK: ${codes.length} codes in ${Object.keys(locales).length} locales`);
}

const realpath = (p) => {
  try {
    return realpathSync(p);
  } catch {
    return null;
  }
};
const self = realpath(fileURLToPath(import.meta.url));
if (process.argv[1] && self !== null && realpath(resolve(process.argv[1])) === self) main();
