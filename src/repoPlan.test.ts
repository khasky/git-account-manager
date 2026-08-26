import { describe, expect, it } from "vitest";
import { decidedByEvidence } from "./repoEvidence";
import { buildRepoPlan } from "./repoPlan";
import type { DiscoveredRepo, RepoBinding, RepoRoot } from "./types";

function root(path: string): RepoRoot {
  return {
    path,
    profile_id: "p1",
    platform: "github",
    install_hook: true,
    pin_remote_alias: false,
  };
}

function repo(overrides: Partial<DiscoveredRepo> & { path: string }) {
  const base: DiscoveredRepo = {
    path: overrides.path,
    name: overrides.path.split("/").pop() ?? "",
    root_path: "/repos",
    remote_url: "git@github.com:octo/demo.git",
    host: "github.com",
    owner: "octo",
    repo: "demo",
    suggested_profile_id: "p1",
    suggested_platform: "github",
    reason: "owner",
    candidate_profile_ids: [],
    bound: false,
    install_hook: true,
    pin_remote_alias: false,
    overrides_root: false,
  };
  return { ...base, ...overrides };
}

function binding(path: string, extra: string[] = []): RepoBinding {
  return {
    path,
    profile_id: "p1",
    platform: "github",
    pin_remote_alias: false,
    install_hook: true,
    extra_allowed_emails: extra,
    overrides_root: false,
  };
}

describe("buildRepoPlan", () => {
  it("binds exactly what the user ticked", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [repo({ path: "/repos/a" }), repo({ path: "/repos/b" })],
      selected: { "/repos/a": true, "/repos/b": false },
      storedBindings: [],
    });

    expect(plan.bindings.map((b) => b.path)).toEqual(["/repos/a"]);
    expect(plan.released).toEqual([]);
  });

  it("releases a repository the scan saw and the user unticked", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [repo({ path: "/repos/a", bound: true })],
      selected: { "/repos/a": false },
      storedBindings: [binding("/repos/a")],
    });

    expect(plan.released).toEqual(["/repos/a"]);
    expect(plan.bindings).toEqual([]);
  });

  // The reason `released` is not simply "everything not selected". An unplugged
  // drive makes a folder scan empty, and dropping those bindings would silently
  // remove the push guard from repositories nobody touched.
  it("keeps a binding whose repository the scan never saw", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [repo({ path: "/repos/a" })],
      selected: { "/repos/a": true },
      storedBindings: [binding("/repos/a"), binding("/repos/offline")],
    });

    expect(plan.released).toEqual([]);
    expect(plan.bindings.map((b) => b.path)).toEqual(["/repos/a"]);
  });

  it("releases a binding whose folder is no longer this profile's", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [],
      selected: {},
      storedBindings: [binding("/elsewhere/old")],
    });

    expect(plan.released).toEqual(["/elsewhere/old"]);
  });

  // A folder path is a prefix, not a substring: `/repos-archive` is a different
  // folder from `/repos` and must not be treated as living inside it.
  it("does not mistake a sibling folder for a child", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [],
      selected: {},
      storedBindings: [binding("/repos-archive/x"), binding("/repos/x")],
    });

    expect(plan.released).toEqual(["/repos-archive/x"]);
  });

  it("carries accepted extra addresses across a re-save", () => {
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [root("/repos")],
      repos: [repo({ path: "/repos/a", bound: true })],
      selected: { "/repos/a": true },
      storedBindings: [binding("/repos/a", ["bot@example.com"])],
    });

    expect(plan.bindings[0].extra_allowed_emails).toEqual(["bot@example.com"]);
  });

  // A repository in an organisation nobody's account owns has no suggested
  // platform; the folder's own setting is what it falls back to.
  it("falls back to the folder's platform when the remote does not name one", () => {
    const gitlabRoot: RepoRoot = { ...root("/repos"), platform: "gitlab" };
    const plan = buildRepoPlan({
      profileId: "p1",
      roots: [gitlabRoot],
      repos: [
        repo({
          path: "/repos/a",
          suggested_platform: null,
          suggested_profile_id: null,
          reason: "unknown",
        }),
      ],
      selected: { "/repos/a": true },
      storedBindings: [],
    });

    expect(plan.bindings[0].platform).toBe("gitlab");
  });
});

describe("decidedByEvidence", () => {
  it("accepts an alias remote and an owned namespace", () => {
    expect(decidedByEvidence(repo({ path: "/a", reason: "alias" }), "p1")).toBe(
      true,
    );
    expect(decidedByEvidence(repo({ path: "/a", reason: "owner" }), "p1")).toBe(
      true,
    );
  });

  // An organisation, a fork, or a namespace two profiles both claim is exactly
  // the case the user has to settle, so it must never pre-tick itself.
  it("refuses to decide anything weaker", () => {
    for (const reason of ["ambiguous", "unknown"] as const) {
      expect(decidedByEvidence(repo({ path: "/a", reason }), "p1")).toBe(false);
    }
  });

  it("refuses evidence that points at a different profile", () => {
    const other = repo({ path: "/a", suggested_profile_id: "p2" });
    expect(decidedByEvidence(other, "p1")).toBe(false);
  });
});
