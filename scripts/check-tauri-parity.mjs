// @ts-check
//
// Usage: node scripts/check-tauri-parity.mjs
//
// Runs the comparison `tauri build` makes, on every push instead of only on a
// tag. Reads the installed package rather than pnpm-lock.yaml: `smoke` installs
// before this step, and the resolved tree is what a build would actually see.

import { readFileSync } from "node:fs";
import { crateVersion, parityError } from "./lib/tauri-parity.mjs";

const crate = crateVersion(readFileSync("src-tauri/Cargo.lock", "utf8"));
const api = JSON.parse(
  readFileSync("node_modules/@tauri-apps/api/package.json", "utf8"),
).version;

const mismatch = parityError(crate, api);
if (mismatch) {
  // The first branch repeats the CLI's own wording so a search for the phrase
  // finds this gate and the release failure it exists to prevent.
  console.error(
    crate
      ? `Found version mismatched Tauri packages: ${mismatch}`
      : `Cannot check Tauri version parity: ${mismatch}`,
  );
  console.error(
    "Align them before tagging - `tauri build` rejects this pair and the release job is where it would surface.",
  );
  process.exit(1);
}

console.log(`tauri ${crate} == @tauri-apps/api ${api}`);
