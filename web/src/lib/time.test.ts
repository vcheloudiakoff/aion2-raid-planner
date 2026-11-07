// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { describe, expect, it } from "vitest";
import { localInputToUtc, timeZoneLabel } from "./time";

describe("localInputToUtc", () => {
  it("converts browser local time to canonical UTC", () => {
    expect(localInputToUtc("2026-11-11T21:00")).toBe("2026-11-11T12:00:00Z");
    expect(localInputToUtc("2026-11-11T05:30")).toBe("2026-11-10T20:30:00Z");
    expect(localInputToUtc("2026-11-11T21:00:15")).toBe("2026-11-11T12:00:15Z");
  });

  it("refuses empty, malformed or impossible values", () => {
    for (const bad of ["", "2026-11-11", "2026-11-11 21:00", "2026-02-30T10:00", "2026-11-11T25:00", "x"]) {
      expect(localInputToUtc(bad), bad).toBeNull();
    }
  });
});

describe("timeZoneLabel", () => {
  it("names the browser zone once, with its short offset", () => {
    const label = timeZoneLabel("en", new Date("2026-11-11T12:00:00Z"));
    expect(label.startsWith("Asia/Tokyo")).toBe(true);
    expect(label).toMatch(/\(.+\)$/);
  });
});
