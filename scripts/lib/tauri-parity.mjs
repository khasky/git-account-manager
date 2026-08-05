// @ts-check
//
// The Rust crate and the npm package are two halves of one product, and the
// Tauri CLI refuses to build when they disagree on major/minor. That check runs
// inside `tauri build`, which only the tag-only `build` job reaches - so a pair
// that drifted apart weeks earlier surfaces as a failed release rather than as
// a failed push. The drift is structural: `Cargo.toml` and `package.json` both
// carry loose ranges, and Dependabot moves the two ecosystems in separate pull
// requests that can land days apart.

/**
 * @param {string} cargoLock contents of src-tauri/Cargo.lock
 * @param {string} crate exact crate name, anchored so `tauri-build` cannot match `tauri`
 * @returns {string | null} the resolved version, or null when the crate is absent
 */
export function crateVersion(cargoLock, crate = "tauri") {
  const found = cargoLock.match(
    new RegExp(`^name = "${crate}"\\r?\\nversion = "([^"]+)"`, "m"),
  );
  return found?.[1] ?? null;
}

/**
 * @param {string | null} crate version of the `tauri` crate
 * @param {string} api version of `@tauri-apps/api`
 * @returns {string | null} the pair the CLI would reject, or null when they agree
 */
export function parityError(crate, api) {
  if (!crate) return 'no "tauri" entry in src-tauri/Cargo.lock';
  const majorMinor = (/** @type {string} */ v) =>
    v.split(".").slice(0, 2).join(".");
  if (majorMinor(crate) === majorMinor(api)) return null;
  return `tauri (v${crate}) : @tauri-apps/api (v${api})`;
}
