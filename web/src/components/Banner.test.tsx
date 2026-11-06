// @vitest-environment happy-dom
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { getJson, resetClientForTests } from "../api/client";
import { resetLangForTests } from "../i18n";
import { byText, click, mount, type Mounted } from "../test/dom";
import { Banners } from "./Banner";

let m: Mounted | null = null;

afterEach(() => {
  m?.unmount();
  m = null;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  resetClientForTests();
});

describe("new-version banner", () => {
  it("reloads only when Reload is clicked", async () => {
    vi.spyOn(navigator, "language", "get").mockReturnValue("fr-FR");
    resetLangForTests();
    resetClientForTests();
    let build = "b1";
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("{}", { status: 200, headers: { "X-App-Build": build } })),
    );
    await getJson("/api/a");
    build = "b2";
    await getJson("/api/a");
    const reload = vi.fn();
    m = mount(<Banners lastDataAt={null} reload={reload} />);
    const button = byText(m.container, "Recharger");
    expect(reload).not.toHaveBeenCalled();
    click(button);
    expect(reload).toHaveBeenCalledTimes(1);
    await act(async () => {});
  });
});
