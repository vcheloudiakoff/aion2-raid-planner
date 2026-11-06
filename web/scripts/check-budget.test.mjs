import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomBytes } from "node:crypto";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { BUDGETS, initialScripts, measure, overBudget } from "./check-budget.mjs";

const script = fileURLToPath(new URL("./check-budget.mjs", import.meta.url));
const dirs = [];

// Un faux build : index.html + fichiers d'assets. Les octets aléatoires ne se compressent pas,
// donc leur taille gzip ≈ leur taille brute.
function dist(html, files) {
  const dir = mkdtempSync(join(tmpdir(), "budget-"));
  dirs.push(dir);
  mkdirSync(join(dir, "assets"));
  writeFileSync(join(dir, "index.html"), html);
  for (const [name, size] of Object.entries(files)) writeFileSync(join(dir, name), randomBytes(size));
  return dir;
}

const HTML = `<!doctype html><html><head>
<script type="module" crossorigin src="/assets/index-a.js"></script>
<link rel="modulepreload" crossorigin href="/assets/vendor-b.js">
<link rel="stylesheet" crossorigin href="/assets/index-c.css">
</head><body><div id="root"></div></body></html>`;

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

describe("initialScripts", () => {
  it("keeps the module entry and modulepreload links only", () => {
    expect(initialScripts(HTML)).toEqual(["assets/index-a.js", "assets/vendor-b.js"]);
  });
});

describe("measure", () => {
  it("sums initial JS and every stylesheet, ignoring lazy chunks", () => {
    const dir = dist(HTML, {
      "assets/index-a.js": 10_000,
      "assets/vendor-b.js": 5_000,
      "assets/lazy-d.js": 400_000,
      "assets/index-c.css": 2_000,
    });
    const sizes = measure(dir);
    expect(sizes.js).toBeGreaterThan(15_000);
    expect(sizes.js).toBeLessThan(16_000);
    expect(sizes.css).toBeGreaterThan(2_000);
    expect(sizes.css).toBeLessThan(2_200);
    expect(overBudget(sizes)).toEqual([]);
  });

  it("fails without a module entry", () => {
    const dir = dist("<html></html>", {});
    expect(() => measure(dir)).toThrow(/entry/);
  });
});

describe("overBudget", () => {
  it("flags each exceeded budget", () => {
    expect(overBudget({ js: BUDGETS.js + 1, css: BUDGETS.css })).toEqual(["js"]);
    expect(overBudget({ js: BUDGETS.js, css: BUDGETS.css + 1 })).toEqual(["css"]);
  });
});

describe("cli", () => {
  it("exits 1 and prints sizes when JS initial exceeds 150 ko gzip", () => {
    const dir = dist(HTML, {
      "assets/index-a.js": 100_000,
      "assets/vendor-b.js": 60_000,
      "assets/index-c.css": 1_000,
    });
    const run = spawnSync(process.execPath, [script, dir], { encoding: "utf8" });
    expect(run.status).toBe(1);
    expect(run.stdout).toContain("JS initial:");
    expect(run.stderr).toContain("budget exceeded: js");
  });

  it("exits 1 when CSS exceeds 20 ko gzip", () => {
    const dir = dist(HTML, { "assets/index-a.js": 1_000, "assets/vendor-b.js": 1_000, "assets/index-c.css": 25_000 });
    const run = spawnSync(process.execPath, [script, dir], { encoding: "utf8" });
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("budget exceeded: css");
  });

  it("still runs when launched through a symlink", () => {
    const dir = dist(HTML, { "assets/index-a.js": 1_000, "assets/vendor-b.js": 1_000, "assets/index-c.css": 25_000 });
    const link = join(dir, "check-budget-link.mjs");
    symlinkSync(script, link);
    const run = spawnSync(process.execPath, [link, dir], { encoding: "utf8" });
    expect(run.status).toBe(1);
    expect(run.stderr).toContain("budget exceeded: css");
  });

  it("exits 0 within budget", () => {
    const dir = dist(HTML, { "assets/index-a.js": 1_000, "assets/vendor-b.js": 1_000, "assets/index-c.css": 1_000 });
    const run = spawnSync(process.execPath, [script, dir], { encoding: "utf8" });
    expect(run.status).toBe(0);
  });
});
