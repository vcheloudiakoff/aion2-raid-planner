// Budget du bundle (AD-19), lancé après `vite build` :
//   node scripts/check-budget.mjs [DIST]
// JS initial = script d'entrée + `modulepreload` de index.html ; CSS = toutes les feuilles du build.
// Tailles gzip ; code 1 au-delà du budget, tailles toujours affichées.

import { readFileSync, readdirSync, realpathSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

export const BUDGETS = { js: 150 * 1024, css: 20 * 1024 };

const attr = (tag, name) => tag.match(new RegExp(`\\s${name}\\s*=\\s*["']([^"']+)["']`, "i"))?.[1];

/** Chemins (relatifs à DIST) du JS chargé au premier rendu, d'après index.html. */
export function initialScripts(html) {
  const paths = new Set();
  for (const tag of html.match(/<script\b[^>]*>/gi) ?? []) {
    const src = attr(tag, "src");
    if (src && /\btype\s*=\s*["']module["']/i.test(tag)) paths.add(src);
  }
  for (const tag of html.match(/<link\b[^>]*>/gi) ?? []) {
    const href = attr(tag, "href");
    if (href && attr(tag, "rel")?.toLowerCase() === "modulepreload") paths.add(href);
  }
  return [...paths].map((p) => p.replace(/^\//, ""));
}

function cssFiles(dir, prefix = "") {
  return readdirSync(join(dir, prefix)).flatMap((name) => {
    const rel = prefix ? `${prefix}/${name}` : name;
    if (statSync(join(dir, rel)).isDirectory()) return cssFiles(dir, rel);
    return rel.endsWith(".css") ? [rel] : [];
  });
}

const gzipSize = (path) => gzipSync(readFileSync(path)).length;

/** Mesure un build : fichiers retenus et totaux gzip, en octets. */
export function measure(dist) {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  const js = initialScripts(html);
  if (js.length === 0) throw new Error("index.html has no module entry script");
  const css = cssFiles(dist);
  const sized = (files) => files.map((file) => ({ file, gzip: gzipSize(join(dist, file)) }));
  const jsFiles = sized(js);
  const cssSized = sized(css);
  const sum = (files) => files.reduce((n, f) => n + f.gzip, 0);
  return { js: sum(jsFiles), css: sum(cssSized), files: [...jsFiles, ...cssSized] };
}

/** Dépassements, vide si le build tient dans le budget. */
export function overBudget(sizes, budgets = BUDGETS) {
  return ["js", "css"].filter((kind) => sizes[kind] > budgets[kind]);
}

const kb = (n) => `${(n / 1024).toFixed(1)} ko`;

function main() {
  const dist = resolve(process.argv[2] ?? join(fileURLToPath(new URL(".", import.meta.url)), "../dist"));
  const sizes = measure(dist);
  for (const { file, gzip } of sizes.files) console.log(`${file}: ${kb(gzip)} gzip`);
  console.log(`JS initial: ${kb(sizes.js)} / ${kb(BUDGETS.js)}`);
  console.log(`CSS: ${kb(sizes.css)} / ${kb(BUDGETS.css)}`);
  const over = overBudget(sizes);
  if (over.length > 0) {
    console.error(`budget exceeded: ${over.join(", ")}`);
    process.exit(1);
  }
}

// Lancé directement, y compris par un lien symbolique ; importé (tests), ne fait rien.
const realpath = (p) => {
  try {
    return realpathSync(p);
  } catch {
    return null;
  }
};
const self = realpath(fileURLToPath(import.meta.url));
if (process.argv[1] && self !== null && realpath(resolve(process.argv[1])) === self) main();
