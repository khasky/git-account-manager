// @ts-check
//
// Usage: node scripts/release-notes.mjs <version> [output]
//
// Writes the release body for one version, read out of CHANGELOG.md. Used by
// the finalize-release job and by the backfill in docs/releasing.md.

import { readFileSync, writeFileSync } from "node:fs";
import { buildReleaseNotes } from "./lib/release-notes.mjs";

const [version, output = "release-notes.md"] = process.argv.slice(2);

if (!version) {
  console.error("Usage: node scripts/release-notes.mjs <version> [output]");
  process.exit(1);
}

writeFileSync(
  output,
  `${buildReleaseNotes(readFileSync("CHANGELOG.md", "utf8"), version)}\n`,
);
