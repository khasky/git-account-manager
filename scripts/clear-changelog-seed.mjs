// @ts-check
//
// The `prechangelog` hook from .versionrc.json - see lib/changelog-seed.mjs for what it prevents.
//
// Usage: node scripts/clear-changelog-seed.mjs [changelog]

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { isSeedOnly } from "./lib/changelog-seed.mjs";

const [file = "CHANGELOG.md"] = process.argv.slice(2);

// The hook runs before commit-and-tag-version creates the file, so an absent one is the normal
// path on a repo that never seeded it, not an error.
if (existsSync(file) && isSeedOnly(readFileSync(file, "utf8"))) writeFileSync(file, "");
