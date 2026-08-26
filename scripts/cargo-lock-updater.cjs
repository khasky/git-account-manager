// commit-and-tag-version updater for src-tauri/Cargo.lock.
//
// Bumps the version of this crate's own [[package]] entry only, identified by
// its `name`. In Cargo.lock each package block lists `name` immediately
// followed by `version`, so we anchor on that adjacency and leave every
// dependency's pinned version untouched.
const NAME = "git-account-manager";
const ENTRY = new RegExp(`(name = "${NAME}"\\r?\\nversion = ")[^"]*(")`);

module.exports.readVersion = function (contents) {
  const match = new RegExp(`name = "${NAME}"\\r?\\nversion = "([^"]*)"`).exec(
    contents,
  );
  return match ? match[1] : undefined;
};

module.exports.writeVersion = function (contents, version) {
  return contents.replace(ENTRY, `$1${version}$2`);
};
