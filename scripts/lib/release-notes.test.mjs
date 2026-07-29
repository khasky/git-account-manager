import { describe, expect, it } from "vitest";
import { buildReleaseNotes } from "./release-notes.mjs";

const CHANGELOG = `# Changelog

## [0.2.0](https://github.com/khasky/git-account-manager/compare/v0.1.0...v0.2.0) (2026-08-27)

### Features

* **repos:** bind folders to the profile that owns them ([575f682](https://x/575f682))

## [0.1.30](https://github.com/khasky/git-account-manager/compare/v0.1.3...v0.1.30) (2026-07-01)

### Bug Fixes

* the thirtieth patch

## [0.1.3](https://github.com/khasky/git-account-manager/compare/v0.1.2...v0.1.3) (2026-06-01)

### Bug Fixes

* the third patch

## 0.1.0 (2026-05-01)

### Features

* the first release
`;

describe("buildReleaseNotes", () => {
  it("returns the version's own section with the compare link as a footer", () => {
    const notes = buildReleaseNotes(CHANGELOG, "0.2.0");

    expect(notes).toContain("### Features");
    expect(notes).toContain("bind folders to the profile that owns them");
    expect(notes).toContain(
      "**Full Changelog**: https://github.com/khasky/git-account-manager/compare/v0.1.0...v0.2.0",
    );
    // The next version's heading ends the slice.
    expect(notes).not.toContain("the thirtieth patch");
  });

  it("does not let a shorter version claim a longer one's section", () => {
    expect(buildReleaseNotes(CHANGELOG, "0.1.3")).toContain("the third patch");
    expect(buildReleaseNotes(CHANGELOG, "0.1.3")).not.toContain(
      "the thirtieth patch",
    );
    expect(buildReleaseNotes(CHANGELOG, "0.1.30")).toContain(
      "the thirtieth patch",
    );
  });

  it("handles a first release, whose heading carries no compare link", () => {
    const notes = buildReleaseNotes(CHANGELOG, "0.1.0");

    expect(notes).toContain("the first release");
    expect(notes).not.toContain("**Full Changelog**");
  });

  it("says what happened when every commit was of a hidden type", () => {
    const silent = `# Changelog

## [0.3.0](https://x/compare/v0.2.0...v0.3.0) (2026-09-01)

## [0.2.0](https://x/compare/v0.1.0...v0.2.0) (2026-08-27)

### Features

* something visible
`;

    expect(buildReleaseNotes(silent, "0.3.0")).toContain(
      "only chore, docs, CI or test commits",
    );
  });

  it("falls back rather than inventing a body for an unknown version", () => {
    expect(buildReleaseNotes(CHANGELOG, "9.9.9")).toBe(
      "See CHANGELOG.md for changes in 9.9.9.",
    );
  });
});
