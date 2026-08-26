// Warning, not error: a scope outside this list is usually a typo or a second
// name for an area that already has one (`repo` for `repos`, `keys` for `ssh`),
// but a genuinely new area must not be blocked from landing.
const scopes = [
  "build",
  "ci",
  "cli",
  "contributing",
  "deps",
  "docs",
  "git",
  "guard",
  "i18n",
  "oauth",
  "openssh",
  "platform",
  "profiles",
  "readme",
  "release",
  "repos",
  "scripts",
  "secrets",
  "security",
  "settings",
  "ssh",
  "storage",
  "tauri",
  "theme",
  "tray",
  "ui",
  "updater",
];

// Both rules read `raw` rather than the parsed `footer`: the parser only splits
// a footer off when it recognizes the trailer, so a `Refs:` line otherwise ends
// up in the body and the check would pass on a commit that has none.
const bangHeader = /^[a-z]+(\([^)]*\))?!:/;
const revertRefs = /^Refs: [0-9a-f]{7,40}\b/m;
const breakingNote = /^BREAKING CHANGE: \S/m;

const rules = {
  // What a revert undid is the one thing prose can't carry: `Refs:` names the
  // commit so `git log --grep` and the changelog can pair the two.
  "revert-refs-footer": ({ type, raw }) => {
    if (type !== "revert" || revertRefs.test(raw ?? "")) return [true];
    return [false, 'revert commits need a "Refs: <sha>" footer naming the reverted commit'];
  },

  // `!` marks that something breaks; it can't say what, and a user reading the
  // changelog needs the what. The footer is what lands under Breaking Changes.
  "breaking-change-footer": ({ header, raw }) => {
    if (!bangHeader.test(header ?? "") || breakingNote.test(raw ?? "")) return [true];
    return [false, 'a "!" header needs a "BREAKING CHANGE: <what breaks>" footer'];
  },
};

/** @type {import("@commitlint/types").UserConfig} */
export default {
  extends: ["@commitlint/config-conventional"],
  plugins: [{ rules }],
  rules: {
    // Trailers can't wrap: an updater endpoint URL, a GHSA id, a commit sha are
    // one token each, so a long one belongs here rather than mid-paragraph. The
    // body keeps the inherited 100 — prose always wraps.
    "footer-max-line-length": [0, "always"],
    "scope-enum": [1, "always", scopes],
    "revert-refs-footer": [2, "always"],
    "breaking-change-footer": [2, "always"],
  },
};
