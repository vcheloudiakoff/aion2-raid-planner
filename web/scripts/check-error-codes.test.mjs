import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { check, parseCodes, readLocales } from "./check-error-codes.mjs";

const script = fileURLToPath(new URL("./check-error-codes.mjs", import.meta.url));
const dirs = [];

function localesDir(files) {
  const dir = mkdtempSync(join(tmpdir(), "locales-"));
  dirs.push(dir);
  for (const [name, content] of Object.entries(files)) writeFileSync(join(dir, name), JSON.stringify(content));
  return dir;
}

const run = (input, dir) => spawnSync(process.execPath, [script, dir], { input, encoding: "utf8" });

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

describe("parseCodes", () => {
  it("drops blank lines, CRLF and duplicates", () => {
    expect(parseCodes("a.b\r\n\nc.d\na.b\n")).toEqual(["a.b", "c.d"]);
  });
});

describe("check", () => {
  const ok = { "errors.a.x": "A", "errors.b.y": "B", "home.empty": "…" };

  it("passes when every code is translated everywhere and nothing is orphaned", () => {
    expect(check(["a.x", "b.y"], { "fr.json": ok, "en.json": ok })).toEqual([]);
  });

  it("names a code missing from one locale", () => {
    const en = { "errors.a.x": "A" };
    expect(check(["a.x", "b.y"], { "fr.json": ok, "en.json": en })).toEqual(["en.json: missing errors.b.y"]);
  });

  it("treats an empty text as missing", () => {
    expect(check(["a.x"], { "fr.json": { "errors.a.x": " " } })).toEqual(["fr.json: missing errors.a.x"]);
  });

  it("names an errors.* key without a code", () => {
    expect(check(["a.x"], { "fr.json": { "errors.a.x": "A", "errors.a.typo": "T" } })).toEqual([
      "fr.json: errors.a.typo has no error code",
    ]);
  });

  it("fails on empty input (cargo failed upstream)", () => {
    expect(check([], { "fr.json": {} })).toEqual(["no error code on stdin"]);
  });
});

describe("cli", () => {
  it("exits 0 when locales match", () => {
    const dir = localesDir({ "fr.json": { "errors.a.x": "A" }, "en.json": { "errors.a.x": "A" } });
    const r = run("a.x\n", dir);
    expect(r.status).toBe(0);
  });

  it("exits 1 and names the code", () => {
    const dir = localesDir({ "fr.json": { "errors.a.x": "A" }, "en.json": {} });
    const r = run("a.x\n", dir);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain("en.json: missing errors.a.x");
  });

  it("checks the repository locales against the same reader", () => {
    const repo = fileURLToPath(new URL("../src/locales", import.meta.url));
    expect(Object.keys(readLocales(repo)).sort()).toEqual(["en.json", "fr.json"]);
  });
});
