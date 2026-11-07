// Fuseau fixé avant tout calcul de date : UTC+9, sans heure d'été.
process.env.TZ = "Asia/Tokyo";

import { describe, expect, it } from "vitest";
import { formatDay, formatSlot, localInputToUtc, timeZoneLabel } from "./time";

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

describe("formatSlot and formatDay", () => {
  it("give the short day and local time of a UTC instant", () => {
    // 2026-11-10T12:00Z = mardi 21:00 à Tokyo.
    expect(formatSlot("2026-11-10T12:00:00Z", "fr")).toBe("mar. 21:00");
    expect(formatSlot("2026-11-10T12:00:00Z", "en")).toBe("Tue 21:00");
    // Le jour suit le fuseau : 2026-11-10T20:00Z est déjà mercredi à Tokyo.
    expect(formatSlot("2026-11-10T20:00:00Z", "fr")).toBe("mer. 05:00");
    const now = new Date("2026-06-01T00:00:00Z");
    expect(formatDay("2026-11-10T12:00:00Z", "fr", now)).toBe("mar. 10 nov.");
    expect(formatDay("2027-01-05T12:00:00Z", "fr", now)).toBe("mar. 5 janv. 2027");
  });
});
