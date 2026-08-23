import type { DiscoveredRepo, RepoBinding, RepoPlan, RepoRoot } from "./types";

interface Draft {
  profileId: string;
  /** The folders this profile will own after Save. */
  roots: RepoRoot[];
  /** What the last scan actually saw. */
  repos: DiscoveredRepo[];
  /** Which of those the user ticked. */
  selected: Record<string, boolean>;
  /** The bindings already on disk for this profile. */
  storedBindings: RepoBinding[];
}

/**
 * Turns the form's draft into the exact set of writes the backend performs.
 *
 * The delicate half is `released`. A stored binding is dropped only when the
 * scan actually saw that repository and the user unticked it, or when its
 * folder left this profile. A repository that was never scanned — an unplugged
 * drive, a folder that failed to walk — keeps its binding: losing a push guard
 * because the disk was not mounted is the one outcome this must never produce.
 */
export function buildRepoPlan({
  profileId,
  roots,
  repos,
  selected,
  storedBindings,
}: Draft): RepoPlan {
  const bindings: RepoBinding[] = repos
    .filter((r) => selected[r.path])
    .map((r) => {
      const previous = storedBindings.find((b) => b.path === r.path);
      const root = roots.find((x) => x.path === r.root_path);
      return {
        path: r.path,
        profile_id: profileId,
        platform: r.suggested_platform ?? root?.platform ?? "github",
        install_hook: r.install_hook,
        pin_remote_alias: r.pin_remote_alias,
        // Addresses the user accepted for this repository are its own record,
        // not something the scan can rediscover, so they survive a re-save.
        extra_allowed_emails: previous?.extra_allowed_emails ?? [],
        overrides_root: r.overrides_root,
      };
    });

  const scanned = new Set(repos.map((r) => r.path));
  const keep = new Set(bindings.map((b) => b.path));
  const released = storedBindings
    .filter(
      (b) =>
        (scanned.has(b.path) && !keep.has(b.path)) ||
        !roots.some(
          (root) => b.path === root.path || b.path.startsWith(`${root.path}/`),
        ),
    )
    .map((b) => b.path);

  return { profile_id: profileId, roots, bindings, released };
}
