// commit-and-tag-version updater for src-tauri/Cargo.toml.
//
// Bumps the [package] version only. That version is the first line-anchored
// `version = "..."` in the file; every dependency uses the inline form
// (`name = { version = "..." }` or `name = "..."`) and is never line-anchored,
// so the multiline regex below can't touch it.
const PACKAGE_VERSION = /^version\s*=\s*"([^"]*)"/m;

module.exports.readVersion = function (contents) {
  const match = PACKAGE_VERSION.exec(contents);
  return match ? match[1] : undefined;
};

module.exports.writeVersion = function (contents, version) {
  return contents.replace(PACKAGE_VERSION, `version = "${version}"`);
};
