import { describe, expect, it } from "vitest";
import en from "./en.json";
import fr from "./fr.json";

const locales: Record<string, Record<string, unknown>> = { fr, en };
const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("locales", () => {
  it("have exactly the same keys", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(fr).sort());
  });

  it("are flat, with no empty value", () => {
    for (const [name, messages] of Object.entries(locales)) {
      for (const [key, value] of Object.entries(messages)) {
        expect(typeof value, `${name} ${key}`).toBe("string");
        expect((value as string).trim(), `${name} ${key}`).not.toBe("");
      }
    }
  });

  it("use the same placeholders in both languages", () => {
    for (const key of Object.keys(fr) as (keyof typeof fr)[]) {
      expect(placeholders(en[key]), key).toEqual(placeholders(fr[key]));
    }
  });

  it("follow the tone: no exclamation mark, no apology", () => {
    for (const [name, messages] of Object.entries(locales)) {
      for (const [key, value] of Object.entries(messages)) {
        expect(value, `${name} ${key}`).not.toMatch(/!|désolé|sorry|veuillez|please/i);
      }
    }
  });
});
