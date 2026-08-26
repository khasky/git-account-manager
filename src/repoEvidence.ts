import type { DiscoveredRepo } from "./types";

/**
 * Evidence strong enough to bind without asking: the remote already carries the
 * profile's SSH alias, or its namespace is that account's own.
 *
 * Anything else — an organisation, a fork, someone else's clone sitting in the
 * folder — waits for a decision rather than being stamped with this identity.
 * Shared by the folder rows, which pre-tick the box, and by the pending count,
 * which must agree with them.
 */
export function decidedByEvidence(
  repo: DiscoveredRepo,
  profileId: string,
): boolean {
  return (
    (repo.reason === "alias" || repo.reason === "owner") &&
    repo.suggested_profile_id === profileId
  );
}
