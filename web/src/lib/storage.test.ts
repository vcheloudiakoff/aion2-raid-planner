// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CANDIDATE_PREFS_KEY,
  readCandidatePrefs,
  readItem,
  readJson,
  removeItem,
  writeCandidatePrefs,
  writeItem,
  writeJson,
} from "./storage";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("candidate prefs", () => {
  it("round-trip pseudo, server and class, and ignore anything else", () => {
    expect(readCandidatePrefs()).toEqual({});
    expect(writeCandidatePrefs({ pseudo: "Nyra", server: "s1", class: "c1" })).toBe(true);
    expect(readCandidatePrefs()).toEqual({ pseudo: "Nyra", server: "s1", class: "c1" });
    localStorage.setItem(CANDIDATE_PREFS_KEY, JSON.stringify({ pseudo: 3, server: "s1", cp: 1500 }));
    expect(readCandidatePrefs()).toEqual({ server: "s1" });
    localStorage.setItem(CANDIDATE_PREFS_KEY, "[");
    expect(readCandidatePrefs()).toEqual({});
  });
});

describe("storage", () => {
  it("reads and writes when storage works", () => {
    expect(writeItem("k", "v")).toBe(true);
    expect(readItem("k")).toBe("v");
    expect(writeJson("j", { a: 1 })).toBe(true);
    expect(readJson("j")).toEqual({ a: 1 });
    removeItem("k");
    expect(readItem("k")).toBeNull();
  });

  it("treats unreadable JSON as absent", () => {
    localStorage.setItem("j", "{oops");
    expect(readJson("j")).toBeNull();
  });

  it("survives a storage whose methods throw", () => {
    const boom = () => {
      throw new DOMException("blocked", "SecurityError");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom, removeItem: boom });
    expect(readItem("k")).toBeNull();
    expect(writeItem("k", "v")).toBe(false);
    expect(() => removeItem("k")).not.toThrow();
    expect(readJson("k")).toBeNull();
    expect(writeJson("k", 1)).toBe(false);
  });

  it("survives a storage whose very access throws", () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() {
        throw new DOMException("denied", "SecurityError");
      },
    });
    try {
      expect(readItem("k")).toBeNull();
      expect(writeItem("k", "v")).toBe(false);
    } finally {
      if (original) Object.defineProperty(globalThis, "localStorage", original);
    }
  });

  it("survives a full storage", () => {
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new DOMException("full", "QuotaExceededError");
    });
    expect(writeItem("k", "v")).toBe(false);
  });
});
