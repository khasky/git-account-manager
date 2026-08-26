// @ts-check
//
// Slices one version's section out of CHANGELOG.md to use as a GitHub release
// body.
//
// GitHub's own `generate-notes` endpoint builds its list from the pull requests
// in the tag range. Everything here lands on `main` as a direct push, so that
// list is always empty and the body came out as nothing but a compare link -
// while CHANGELOG.md, written by commit-and-tag-version from the same commits,
// already held exactly the text the release page was missing.
//
// The compare link lives in the `##` version heading the slice starts *after*,
// so it is lifted out separately and re-appended as a footer - otherwise the
// release page carries no link to the diff it shipped.

/**
 * A heading belongs to the version when nothing, whitespace, or the `(` of the
 * date suffix follows it - `0.1.3` must not claim `0.1.30`'s section.
 * @param {string} heading normalized heading text
 * @param {string} version
 */
function headingIsVersion(heading, version) {
  if (!heading.startsWith(version)) return false;
  const rest = heading.slice(version.length);
  return rest === "" || rest.startsWith("(") || /^\s/.test(rest);
}

/** @param {string} text */
function normalizeHeading(text) {
  return text
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/^v/, "")
    .trim();
}

/**
 * @param {string} changelog contents of CHANGELOG.md
 * @param {string} version release version without the `v` prefix, e.g. `0.2.0`
 * @returns {string} the release body, without a trailing newline
 */
export function buildReleaseNotes(changelog, version) {
  const lines = changelog.split(/\r?\n/);

  let start = -1;
  let end = lines.length;
  let compareUrl = "";

  for (let i = 0; i < lines.length; i++) {
    const match = /^##\s+(.+?)\s*$/.exec(lines[i]);
    if (!match) continue;

    if (start === -1 && headingIsVersion(normalizeHeading(match[1]), version)) {
      start = i + 1;
      compareUrl =
        /\]\((https:\/\/[^)\s]+\/compare\/[^)\s]+)\)/.exec(match[1])?.[1] ?? "";
      continue;
    }

    if (start !== -1) {
      end = i;
      break;
    }
  }

  if (start === -1) return `See CHANGELOG.md for changes in ${version}.`;

  const section = lines.slice(start, end).join("\n").trim();
  // A version whose commits are all hidden types renders an empty section.
  // Pointing readers at a file that is equally silent there says nothing, so
  // name what happened instead.
  const body =
    section ||
    "No changelog-visible changes in this release - it carries only chore, docs, CI or test commits.";

  return compareUrl ? `${body}\n\n**Full Changelog**: ${compareUrl}` : body;
}
