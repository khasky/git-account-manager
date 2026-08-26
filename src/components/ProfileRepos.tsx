import { Dispatch, SetStateAction, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  DiscoveredRepo,
  PlatformId,
  Profile,
  RepoCheck,
  RepoReach,
  RepoRoot,
  RepoStatus,
} from "../types";
import Toggle from "./Toggle";
import { useI18n, fmt } from "../i18n";

interface Props {
  /** The profile as edited, which may not exist on disk yet. */
  profile: Profile;
  platforms: PlatformId[];
  roots: RepoRoot[];
  setRoots: Dispatch<SetStateAction<RepoRoot[]>>;
  repos: DiscoveredRepo[];
  setRepos: Dispatch<SetStateAction<DiscoveredRepo[]>>;
  selected: Record<string, boolean>;
  setSelected: Dispatch<SetStateAction<Record<string, boolean>>>;
  /** Doctor rows belonging to this profile. */
  statuses: RepoStatus[];
  onFixed: () => void;
}

/** Evidence strong enough to bind without asking: the remote already carries the
 *  profile's SSH alias, or its namespace is that account's own. Anything else —
 *  an organisation, a fork, someone else's clone sitting in the folder — waits
 *  for a decision rather than being stamped with this identity. */
function decidedByEvidence(repo: DiscoveredRepo, profileId: string): boolean {
  return (
    (repo.reason === "alias" || repo.reason === "owner") &&
    repo.suggested_profile_id === profileId
  );
}

export default function ProfileRepos({
  profile,
  platforms,
  roots,
  setRoots,
  repos,
  setRepos,
  selected,
  setSelected,
  statuses,
  onFixed,
}: Props) {
  const { m } = useI18n();
  const [busy, setBusy] = useState("");
  const [note, setNote] = useState("");
  const [openRoots, setOpenRoots] = useState<Record<string, boolean>>({});
  const autoScanned = useRef(false);

  // Opening a profile that already has folders must show what is in them.
  // Without this the summary reads "0 repositories" over a folder holding a
  // dozen bound ones, which is not a neutral empty state — it is wrong.
  useEffect(() => {
    if (autoScanned.current || roots.length === 0) return;
    autoScanned.current = true;
    scanWith(roots);
    // scanWith is stable enough for a one-shot mount scan; re-running it on
    // every roots edit would walk the disk on each toggle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [roots]);

  async function run<T>(key: string, task: () => Promise<T>): Promise<T | null> {
    setBusy(key);
    setNote("");
    try {
      return await task();
    } catch (e) {
      setNote(fmt(m.repos.error, { error: String(e) }));
      return null;
    } finally {
      setBusy("");
    }
  }

  async function scanWith(next: RepoRoot[]) {
    const found = await run("scan", () =>
      invoke<DiscoveredRepo[]>("scan_profile_repositories", {
        profile,
        roots: next,
      }),
    );
    if (!found) return;
    setRepos(found);
    setSelected(
      Object.fromEntries(
        found.map((r) => [
          r.path,
          r.bound || decidedByEvidence(r, profile.id),
        ]),
      ),
    );
    setOpenRoots(Object.fromEntries(next.map((r) => [r.path, true])));
  }

  async function addFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    const path = picked.replace(/\\/g, "/");
    if (roots.some((r) => r.path === path)) return;
    const next: RepoRoot[] = [
      ...roots,
      {
        path,
        profile_id: profile.id,
        platform: profile.default_platform ?? platforms[0] ?? "github",
        install_hook: true,
        pin_remote_alias: false,
      },
    ];
    setRoots(next);
    await scanWith(next);
  }

  function removeFolder(path: string) {
    setRoots((prev) => prev.filter((r) => r.path !== path));
    setRepos((prev) => prev.filter((r) => r.root_path !== path));
  }

  /** A folder's defaults reach every repository under it that the user has not
   *  deliberately set apart. Mirrors `repos::effective_switches` in the backend,
   *  which resolves the same rule when a scan first materialises these rows. */
  function updateFolder(path: string, next: Partial<RepoRoot>) {
    setRoots((prev) =>
      prev.map((r) => (r.path === path ? { ...r, ...next } : r)),
    );
    setRepos((prev) =>
      prev.map((r) =>
        r.root_path === path && !r.overrides_root
          ? {
              ...r,
              install_hook: next.install_hook ?? r.install_hook,
              pin_remote_alias: next.pin_remote_alias ?? r.pin_remote_alias,
            }
          : r,
      ),
    );
  }

  function overrideRepo(path: string, next: Partial<DiscoveredRepo>) {
    setRepos((prev) =>
      prev.map((r) =>
        r.path === path ? { ...r, ...next, overrides_root: true } : r,
      ),
    );
  }

  async function checkAccess(repo: DiscoveredRepo) {
    const platform =
      (repo.suggested_platform as PlatformId | null) ??
      (platforms.length === 1 ? platforms[0] : null);
    if (!platform) return;
    const access = await run(`access:${repo.path}`, () =>
      invoke<RepoReach>("verify_repo_access", {
        profileId: profile.id,
        platform,
        owner: repo.owner,
        repo: repo.repo,
      }),
    );
    if (!access) return;
    setNote(
      access.reachable
        ? fmt(m.repos.accessOk, { full: access.full_name })
        : fmt(m.repos.accessDenied, {
            full: access.full_name,
            detail: access.detail,
          }),
    );
  }

  async function probeAlias(host: string) {
    const answer = await run(`ssh:${host}`, () =>
      invoke<string>("probe_ssh_alias", { host }),
    );
    if (answer) setNote(answer);
  }

  async function fixRepo(path: string) {
    const done = await run(`fix:${path}`, () =>
      invoke("fix_repository", { path }),
    );
    if (done === null) return;
    setNote(m.repos.fixed);
    onFixed();
  }

  async function allowEmail(path: string, email: string) {
    const done = await run(`allow:${path}`, () =>
      invoke("allow_email_in_repository", { path, email }),
    );
    if (done === null) return;
    onFixed();
  }

  const checkLabel: Record<RepoCheck["id"], string> = {
    exists: m.repos.checkExists,
    identity: m.repos.checkIdentity,
    local: m.repos.checkLocal,
    remote: m.repos.checkRemote,
    history: m.repos.checkHistory,
    hooks: m.repos.checkHooks,
  };

  const hookLabel: Record<string, string> = {
    installed: m.repos.hookInstalled,
    "kept-existing": m.repos.hookKeptExisting,
    unavailable: m.repos.hookUnavailable,
    off: m.repos.hookOff,
    missing: m.repos.hookMissing,
  };

  const reasonLabel: Record<DiscoveredRepo["reason"], string> = {
    alias: m.repos.reasonAlias,
    owner: m.repos.reasonOwner,
    ambiguous: m.repos.reasonAmbiguous,
    unknown: m.repos.reasonUnknown,
  };

  const problems = statuses.filter((s) => !s.ok);

  return (
    <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h4 className="font-medium text-fg-2">{m.repos.sectionTitle}</h4>
          <p className="text-xs text-fg-5">{m.repos.sectionHint}</p>
        </div>
        <div className="flex shrink-0 gap-2">
          {roots.length > 0 && (
            <button
              type="button"
              onClick={() => scanWith(roots)}
              disabled={busy === "scan"}
              className="rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-subtle disabled:opacity-50"
            >
              {busy === "scan" ? m.repos.scanning : m.repos.rescan}
            </button>
          )}
          <button
            type="button"
            onClick={addFolder}
            className="rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-subtle"
          >
            {m.repos.addFolder}
          </button>
        </div>
      </div>

      {roots.length === 0 ? (
        <p className="text-xs text-fg-5">{m.repos.noRoots}</p>
      ) : (
        <ul className="space-y-3">
          {roots.map((root) => {
            const inFolder = repos.filter((r) => r.root_path === root.path);
            const chosen = inFolder.filter((r) => selected[r.path]).length;
            const pending = inFolder.filter(
              (r) => !decidedByEvidence(r, profile.id) && !r.bound,
            ).length;
            const isOpen = openRoots[root.path] ?? false;
            return (
              <li
                key={root.path}
                className="space-y-2 rounded-md bg-raised p-3"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <code className="flex-1 truncate text-xs text-fg-3">
                    {root.path}
                  </code>
                  {platforms.length > 1 && (
                    <select
                      value={root.platform}
                      onChange={(e) =>
                        updateFolder(root.path, { platform: e.target.value })
                      }
                      className="rounded-md border border-bd-s bg-input px-2 py-1 text-xs text-fg outline-none focus:border-blue-500"
                    >
                      {platforms.map((p) => (
                        <option key={p} value={p}>
                          {p}
                        </option>
                      ))}
                    </select>
                  )}
                  <button
                    type="button"
                    onClick={() => removeFolder(root.path)}
                    className="rounded-md px-2 py-1 text-xs text-fg-4 hover:text-red-500"
                  >
                    {m.repos.remove}
                  </button>
                </div>

                <div className="space-y-1.5 border-t border-bd pt-2">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-xs text-fg-3">{m.repos.installHook}</p>
                      <p className="text-[11px] text-fg-5">
                        {m.repos.installHookHint}
                      </p>
                    </div>
                    <Toggle
                      size="sm"
                      on={root.install_hook}
                      onClick={() =>
                        updateFolder(root.path, {
                          install_hook: !root.install_hook,
                        })
                      }
                    />
                  </div>
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-xs text-fg-3">{m.repos.pinAlias}</p>
                      <p className="text-[11px] text-fg-5">
                        {m.repos.pinAliasHint}
                      </p>
                    </div>
                    <Toggle
                      size="sm"
                      on={root.pin_remote_alias}
                      onClick={() =>
                        updateFolder(root.path, {
                          pin_remote_alias: !root.pin_remote_alias,
                        })
                      }
                    />
                  </div>
                </div>

                <button
                  type="button"
                  onClick={() =>
                    setOpenRoots((prev) => ({
                      ...prev,
                      [root.path]: !isOpen,
                    }))
                  }
                  className="flex w-full items-center gap-1 border-t border-bd pt-2 text-left text-[11px] text-fg-4 hover:text-fg-2"
                >
                  <span>{isOpen ? "▾" : "▸"}</span>
                  <span>
                    {fmt(m.repos.folderSummary, {
                      total: inFolder.length,
                      chosen,
                    })}
                    {pending > 0 && ` · ${fmt(m.repos.pending, { pending })}`}
                  </span>
                </button>

                {isOpen && inFolder.length > 0 && (
                  <ul className="space-y-2">
                    {inFolder.map((repo) => {
                      const needsDecision =
                        !decidedByEvidence(repo, profile.id) && !repo.bound;
                      return (
                        <li
                          key={repo.path}
                          className={`space-y-1.5 rounded-md p-2 ${
                            needsDecision
                              ? "bg-raised-40 ring-1 ring-amber-500/40"
                              : "bg-raised-40"
                          }`}
                        >
                          <div className="flex flex-wrap items-center gap-2">
                            <input
                              type="checkbox"
                              checked={selected[repo.path] ?? false}
                              onChange={(e) =>
                                setSelected((prev) => ({
                                  ...prev,
                                  [repo.path]: e.target.checked,
                                }))
                              }
                              className="h-3.5 w-3.5 accent-blue-600"
                            />
                            <span className="text-xs font-medium text-fg-2">
                              {repo.name}
                            </span>
                            {repo.overrides_root && (
                              <span className="rounded bg-raised px-1.5 py-0.5 text-[10px] text-fg-4">
                                {m.repos.overridden}
                              </span>
                            )}
                          </div>
                          <p className="text-[11px] text-fg-5">
                            {reasonLabel[repo.reason]}
                          </p>
                          <code className="block truncate text-[11px] text-fg-4">
                            {repo.remote_url}
                          </code>

                          <div className="flex flex-wrap items-center gap-3 pt-0.5">
                            <label className="flex items-center gap-1 text-[11px] text-fg-4">
                              <input
                                type="checkbox"
                                checked={repo.install_hook}
                                onChange={(e) =>
                                  overrideRepo(repo.path, {
                                    install_hook: e.target.checked,
                                  })
                                }
                                className="h-3 w-3 accent-blue-600"
                              />
                              {m.repos.hookShort}
                            </label>
                            <label className="flex items-center gap-1 text-[11px] text-fg-4">
                              <input
                                type="checkbox"
                                checked={repo.pin_remote_alias}
                                onChange={(e) =>
                                  overrideRepo(repo.path, {
                                    pin_remote_alias: e.target.checked,
                                  })
                                }
                                className="h-3 w-3 accent-blue-600"
                              />
                              {m.repos.aliasShort}
                            </label>
                            <button
                              type="button"
                              onClick={() => checkAccess(repo)}
                              disabled={busy === `access:${repo.path}`}
                              className="text-[11px] text-link hover:underline disabled:opacity-50"
                            >
                              {m.repos.verifyAccess}
                            </button>
                            <button
                              type="button"
                              onClick={() => probeAlias(repo.host)}
                              disabled={busy === `ssh:${repo.host}`}
                              className="text-[11px] text-link hover:underline disabled:opacity-50"
                            >
                              {m.repos.probeAlias}
                            </button>
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {problems.length > 0 && (
        <div className="space-y-2 border-t border-bd pt-3">
          <div>
            <h4 className="text-xs font-medium text-fg-2">
              {m.repos.doctorTitle}
            </h4>
            <p className="text-[11px] text-fg-5">{m.repos.doctorHint}</p>
          </div>
          <ul className="space-y-2">
            {problems.map((repo) => (
              <li key={repo.path} className="rounded-md bg-raised p-3">
                <div className="flex flex-wrap items-baseline justify-between gap-2">
                  <span className="text-xs font-medium text-fg-2">
                    {repo.name}
                  </span>
                  <span className="text-[11px] text-fg-5">
                    {repo.expected_email}
                  </span>
                </div>
                <ul className="mt-2 space-y-1">
                  {repo.checks
                    .filter((c) => !c.ok)
                    .map((c) => (
                      <li
                        key={c.id}
                        className="flex items-baseline justify-between gap-3 text-[11px]"
                      >
                        <span className="text-red-600 dark:text-red-400">
                          ✗ {checkLabel[c.id]}
                        </span>
                        <span className="truncate text-right text-fg-4">
                          {c.id === "hooks"
                            ? (hookLabel[c.detail] ?? c.detail)
                            : c.detail}
                        </span>
                      </li>
                    ))}
                </ul>
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={() => fixRepo(repo.path)}
                    disabled={busy === `fix:${repo.path}`}
                    className="rounded-md bg-blue-600 px-3 py-1 text-[11px] font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                  >
                    {m.repos.fix}
                  </button>
                  {repo.offending_emails.map((email) => (
                    <button
                      type="button"
                      key={email}
                      onClick={() => allowEmail(repo.path, email)}
                      className="rounded-md bg-raised-40 px-3 py-1 text-[11px] text-fg-3 hover:bg-subtle"
                    >
                      {m.repos.allowEmail}: {email}
                    </button>
                  ))}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {note && (
        <p className="border-t border-bd pt-2 text-[11px] break-all whitespace-pre-wrap text-fg-3">
          {note}
        </p>
      )}
    </div>
  );
}
