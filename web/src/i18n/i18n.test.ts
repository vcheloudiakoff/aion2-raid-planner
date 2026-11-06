// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LANG_KEY,
  detectLang,
  getLang,
  hasKey,
  initialLang,
  resetLangForTests,
  setLang,
  subscribeLang,
  t,
  translate,
} from ".";

function browserLanguage(tag: string) {
  vi.spyOn(navigator, "language", "get").mockReturnValue(tag);
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("detectLang", () => {
  it("maps BCP 47 tags to fr or en, unknown to fr", () => {
    expect(detectLang("en-US")).toBe("en");
    expect(detectLang("EN_gb")).toBe("en");
    expect(detectLang("fr-CA")).toBe("fr");
    expect(detectLang("de-DE")).toBe("fr");
    expect(detectLang("")).toBe("fr");
    expect(detectLang(undefined)).toBe("fr");
  });
});

describe("initial language", () => {
  it("follows navigator.language when nothing is chosen", () => {
    browserLanguage("en-US");
    resetLangForTests();
    expect(getLang()).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(t("home.empty")).toBe("No raids open right now.");
  });

  it("prefers the remembered choice", () => {
    browserLanguage("en-US");
    localStorage.setItem(LANG_KEY, "fr");
    expect(initialLang()).toBe("fr");
  });

  it("ignores a garbage stored value", () => {
    browserLanguage("en-US");
    localStorage.setItem(LANG_KEY, "klingon");
    expect(initialLang()).toBe("en");
  });

  it("still works when storage throws", () => {
    browserLanguage("en-US");
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    resetLangForTests();
    expect(getLang()).toBe("en");
    setLang("fr");
    expect(getLang()).toBe("fr");
    expect(document.documentElement.lang).toBe("fr");
  });
});

describe("setLang", () => {
  it("remembers the choice, updates <html lang> and notifies", () => {
    browserLanguage("fr-FR");
    resetLangForTests();
    const seen: string[] = [];
    const off = subscribeLang(() => seen.push(getLang()));
    setLang("en");
    off();
    expect(seen).toEqual(["en"]);
    expect(localStorage.getItem(LANG_KEY)).toBe("en");
    expect(document.documentElement.lang).toBe("en");
  });
});

describe("translate", () => {
  it("replaces placeholders and keeps unknown ones", () => {
    expect(translate("fr", "banner.offlineSince", { time: "21:04" })).toBe(
      "Hors ligne · dernières données de 21:04",
    );
    expect(translate("en", "banner.offlineSince", {})).toBe("Offline · last data from {time}");
  });

  it("returns the key itself when it does not exist", () => {
    expect(translate("en", "nope.missing")).toBe("nope.missing");
    expect(hasKey("nope.missing")).toBe(false);
    expect(hasKey("errors.server.internal")).toBe(true);
  });
});
