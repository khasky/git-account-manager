// @ts-check
//
// This guard fires once per repo lifetime - at the first release, where a wrong verdict either
// duplicates the header or wipes a real changelog.
import { describe, expect, it } from "vitest";
import { isSeedOnly } from "./changelog-seed.mjs";

const seed =
  "# Changelog\n\nAll notable changes to Git Account Manager are documented here.\n";

describe("isSeedOnly", () => {
  it("reports a hand-written header that no release has been added to yet", () => {
    expect(isSeedOnly(seed)).toBe(true);
  });

  it("keeps a changelog that already carries a release heading", () => {
    expect(
      isSeedOnly(`${seed}\n## 1.0.0 (2026-08-27)\n\n### Features\n\n* first thing\n`),
    ).toBe(false);
  });

  it("keeps the anchor form standard-version used to write", () => {
    expect(isSeedOnly(`${seed}\n<a name="1.0.0"></a>\n`)).toBe(false);
  });

  it("leaves a file with nothing to lose alone", () => {
    expect(isSeedOnly("")).toBe(false);
    expect(isSeedOnly("\n")).toBe(false);
  });
});
