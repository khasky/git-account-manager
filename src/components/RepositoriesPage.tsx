import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BindResult,
  DiscoveredRepo,
  DoctorReport,
  Profile,
  RepoAccess,
  RepoBinding,
  RepoCheck,
  RepoRoot,
  RepoState,
} from "../types";
import { useI18n, fmt } from "../i18n";

interface Props {
  onBack: () => void;
}

const PLATFORMS = ["github", "gitlab", "bitbucket"] as const;
type PlatformId = (typeof PLATFORMS)[number];

interface Draft {
  profileId: string;
  pinAlias: boolean;
  installHook: boolean;
}

function platformsOf(profile: Profile): PlatformId[] {
  return PLATFORMS.filter((p) => profile[p] !== undefined && profile[p] !== null);
}

/** The host tells us the platform; a self-hosted host does not, so fall back to
 *  the profile's only account rather than guessing between several. */
function resolvePlatform(
  repo: DiscoveredRepo,
  profile: Profile | undefined,
): PlatformId | null {
  if (repo.suggested_platform) return repo.suggested_platform as PlatformId;
  if (!profile) return null;
  const owned = platformsOf(profile);
  return owned.length === 1 ? owned[0] : null;
}

function Toggle({
  on,
  onClick,
}: {
  on: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={on}
      className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
        on ? "bg-emerald-600" : "bg-toggle-off"
      }`}
    >
      <span
        className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
          on ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </button>
  );
}

export default function RepositoriesPage({ onBack }: Props) {
  const { m } = useI18n();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [state, setState] = useState<RepoState | null>(null);
  const [found, setFound] = useState<DiscoveredRepo[]>([]);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [report, setReport] = useState<DoctorReport | null>(null);
  const [busy, setBusy] = useState("");
  const [note, setNote] = useState("");

  const reload = useCallback(async () => {
    const [p, s, r] = await Promise.all([
      invoke<Profile[]>("get_profiles"),
      invoke<RepoState>("get_repo_state"),
      invoke<DoctorReport>("doctor"),
    ]);
    setProfiles(p);
    setState(s);
    setReport(r);
  }, []);

  useEffect(() => {
    reload().catch((e) => setNote(fmt(m.repos.error, { error: String(e) })));
  }, [reload, m.repos.error]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onBack]);

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

  async function addFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string" || !state) return;
    const path = picked.replace(/\\/g, "/");
    if (state.roots.some((r) => r.path === path)) return;
    const profile = profiles[0];
    if (!profile) return;
    const platform = platformsOf(profile)[0] ?? "github";
    const roots = [...state.roots, { path, profile_id: profile.id, platform }];
    await run("roots", async () => {
      await invoke("save_repo_roots", { roots });
      await reload();
    });
  }

  async function updateRoot(path: string, next: Partial<RepoRoot>) {
    if (!state) return;
    const roots = state.roots.map((r) =>
      r.path === path ? { ...r, ...next } : r,
    );
    await run("roots", async () => {
      await invoke("save_repo_roots", { roots });
      await reload();
    });
  }

  async function removeRoot(path: string) {
    if (!state) return;
    const roots = state.roots.filter((r) => r.path !== path);
    await run("roots", async () => {
      await invoke("save_repo_roots", { roots });
      await reload();
    });
  }

  async function scan() {
    const repos = await run("scan", () =>
      invoke<DiscoveredRepo[]>("scan_repositories"),
    );
    if (!repos) return;
    setFound(repos);
    setDrafts((prev) => {
      const next = { ...prev };
      for (const repo of repos) {
        if (next[repo.path]) continue;
        const bound = state?.bindings.find((b) => b.path === repo.path);
        next[repo.path] = {
          profileId:
            bound?.profile_id ??
            repo.suggested_profile_id ??
            repo.candidate_profile_ids[0] ??
            "",
          pinAlias: bound?.pin_remote_alias ?? false,
          installHook: bound?.install_hook ?? true,
        };
      }
      return next;
    });
  }

  async function bind(repo: DiscoveredRepo) {
    const draft = drafts[repo.path];
    const profile = profiles.find((p) => p.id === draft?.profileId);
    const platform = resolvePlatform(repo, profile);
    if (!draft || !profile || !platform) return;
    const existing = state?.bindings.find((b) => b.path === repo.path);
    const binding: RepoBinding = {
      path: repo.path,
      profile_id: profile.id,
      platform,
      pin_remote_alias: draft.pinAlias,
      install_hook: draft.installHook,
      extra_allowed_emails: existing?.extra_allowed_emails ?? [],
    };
    const result = await run(repo.path, () =>
      invoke<BindResult>("bind_repository", { binding }),
    );
    if (!result) return;
    setNote(
      result.remote_url
        ? `${result.identity} · ${result.remote_url}`
        : result.identity,
    );
    await reload();
    await scan();
  }

  async function unbind(path: string) {
    await run(path, async () => {
      await invoke("unbind_repository", { path });
      await reload();
    });
    await scan();
  }

  async function checkAccess(repo: DiscoveredRepo) {
    const draft = drafts[repo.path];
    const profile = profiles.find((p) => p.id === draft?.profileId);
    const platform = resolvePlatform(repo, profile);
    if (!profile || !platform) return;
    const access = await run(`access:${repo.path}`, () =>
      invoke<RepoAccess>("verify_repo_access", {
        profileId: profile.id,
        platform,
        owner: repo.owner,
        repo: repo.repo,
      }),
    );
    if (!access) return;
    const template = !access.found
      ? m.repos.accessMissing
      : access.can_push
        ? m.repos.accessOk
        : m.repos.accessNoPush;
    setNote(fmt(template, { full: access.full_name }));
  }

  async function probeAlias(host: string) {
    const answer = await run(`ssh:${host}`, () =>
      invoke<string>("probe_ssh_alias", { host }),
    );
    if (answer) setNote(answer);
  }

  async function fixRepo(path: string) {
    const result = await run(`fix:${path}`, () =>
      invoke<BindResult>("fix_repository", { path }),
    );
    if (!result) return;
    setNote(m.repos.fixed);
    await reload();
  }

  async function allowEmail(path: string, email: string) {
    await run(`allow:${path}`, async () => {
      await invoke("allow_email_in_repository", { path, email });
      await reload();
    });
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

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 border-b border-bd px-6 py-4">
        <button onClick={onBack} className="text-fg-4 hover:text-fg-2">
          <svg
            className="h-5 w-5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
        </button>
        <div>
          <h2 className="text-lg font-semibold text-fg">{m.repos.title}</h2>
          <p className="text-xs text-fg-5">{m.repos.subtitle}</p>
        </div>
      </div>

      <div className="flex-1 space-y-6 overflow-y-auto p-6">
        {/* Machine-wide state */}
        {report && (
          <div className="space-y-2 rounded-lg border border-bd bg-raised-40 p-4">
            <h3 className="font-medium text-fg-2">{m.repos.machineTitle}</h3>
            <dl className="space-y-1 text-xs">
              <div className="flex justify-between gap-4">
                <dt className="text-fg-4">{m.repos.globalIdentity}</dt>
                <dd className="text-right text-fg-3">
                  {report.guard.global_email || m.repos.globalNone}
                </dd>
              </div>
              <div className="flex justify-between gap-4">
                <dt className="text-fg-4">{m.repos.useConfigOnly}</dt>
                <dd className="text-fg-3">
                  {report.guard.use_config_only ? m.repos.on : m.repos.off}
                </dd>
              </div>
              <div className="flex justify-between gap-4">
                <dt className="text-fg-4">{m.repos.includesRegion}</dt>
                <dd className="text-fg-3">
                  {report.guard.includes_managed ? m.repos.on : m.repos.off}
                </dd>
              </div>
            </dl>
          </div>
        )}

        {/* Watched folders */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="font-medium text-fg-2">{m.repos.rootsTitle}</h3>
              <p className="text-xs text-fg-5">{m.repos.rootsHint}</p>
            </div>
            <button
              onClick={addFolder}
              disabled={profiles.length === 0}
              className="shrink-0 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-subtle disabled:opacity-50"
            >
              {m.repos.addFolder}
            </button>
          </div>

          {state && state.roots.length === 0 ? (
            <p className="text-xs text-fg-5">{m.repos.noRoots}</p>
          ) : (
            <ul className="space-y-2">
              {state?.roots.map((root) => {
                const profile = profiles.find((p) => p.id === root.profile_id);
                return (
                  <li
                    key={root.path}
                    className="flex flex-wrap items-center gap-2 rounded-md bg-raised px-3 py-2"
                  >
                    <code className="flex-1 truncate text-xs text-fg-3">
                      {root.path}
                    </code>
                    <select
                      value={root.profile_id}
                      onChange={(e) => {
                        const next = profiles.find(
                          (p) => p.id === e.target.value,
                        );
                        updateRoot(root.path, {
                          profile_id: e.target.value,
                          platform: next
                            ? (platformsOf(next)[0] ?? root.platform)
                            : root.platform,
                        });
                      }}
                      className="rounded-md border border-bd-s bg-input px-2 py-1 text-xs text-fg outline-none focus:border-blue-500"
                    >
                      {profiles.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                    <select
                      value={root.platform}
                      onChange={(e) =>
                        updateRoot(root.path, { platform: e.target.value })
                      }
                      className="rounded-md border border-bd-s bg-input px-2 py-1 text-xs text-fg outline-none focus:border-blue-500"
                    >
                      {(profile ? platformsOf(profile) : PLATFORMS).map((p) => (
                        <option key={p} value={p}>
                          {p}
                        </option>
                      ))}
                    </select>
                    <button
                      onClick={() => removeRoot(root.path)}
                      className="rounded-md px-2 py-1 text-xs text-fg-4 hover:text-red-500"
                    >
                      {m.repos.remove}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}

          <button
            onClick={scan}
            disabled={!state || state.roots.length === 0 || busy === "scan"}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
          >
            {busy === "scan" ? m.repos.scanning : m.repos.scan}
          </button>
        </div>

        {/* Discovered repositories */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <h3 className="font-medium text-fg-2">{m.repos.reposTitle}</h3>
          {found.length === 0 ? (
            <p className="text-xs text-fg-5">{m.repos.noRepos}</p>
          ) : (
            <ul className="space-y-3">
              {found.map((repo) => {
                const draft = drafts[repo.path];
                const profile = profiles.find((p) => p.id === draft?.profileId);
                const platform = resolvePlatform(repo, profile);
                const eligible = profiles.filter((p) =>
                  repo.suggested_platform
                    ? p[repo.suggested_platform as PlatformId]
                    : platformsOf(p).length > 0,
                );
                return (
                  <li
                    key={repo.path}
                    className="space-y-2 rounded-md bg-raised p-3"
                  >
                    <div className="flex flex-wrap items-baseline justify-between gap-2">
                      <span className="text-sm font-medium text-fg-2">
                        {repo.name}
                      </span>
                      {repo.bound && (
                        <span className="rounded bg-emerald-600/15 px-2 py-0.5 text-[11px] text-emerald-600 dark:text-emerald-400">
                          {m.repos.bound}
                        </span>
                      )}
                    </div>
                    <code className="block truncate text-[11px] text-fg-5">
                      {repo.path}
                    </code>
                    <code className="block truncate text-[11px] text-fg-4">
                      {repo.remote_url}
                    </code>
                    <p className="text-[11px] text-fg-5">
                      {reasonLabel[repo.reason]}
                    </p>

                    <div className="flex flex-wrap items-center gap-2">
                      <select
                        value={draft?.profileId ?? ""}
                        onChange={(e) =>
                          setDrafts((prev) => ({
                            ...prev,
                            [repo.path]: {
                              ...(prev[repo.path] ?? {
                                pinAlias: false,
                                installHook: true,
                                profileId: "",
                              }),
                              profileId: e.target.value,
                            },
                          }))
                        }
                        className="rounded-md border border-bd-s bg-input px-2 py-1 text-xs text-fg outline-none focus:border-blue-500"
                      >
                        <option value="">{m.repos.chooseProfile}</option>
                        {eligible.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.name}
                          </option>
                        ))}
                      </select>

                      <button
                        onClick={() => bind(repo)}
                        disabled={!platform || !profile || busy === repo.path}
                        className="rounded-md bg-blue-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                      >
                        {repo.bound ? m.repos.rebind : m.repos.bind}
                      </button>
                      {repo.bound && (
                        <button
                          onClick={() => unbind(repo.path)}
                          className="rounded-md px-2 py-1.5 text-xs text-fg-4 hover:text-red-500"
                        >
                          {m.repos.unbind}
                        </button>
                      )}
                      <button
                        onClick={() => checkAccess(repo)}
                        disabled={!platform || !profile}
                        className="rounded-md bg-raised-40 px-3 py-1.5 text-xs text-fg-3 hover:bg-subtle disabled:opacity-50"
                      >
                        {m.repos.verifyAccess}
                      </button>
                      <button
                        onClick={() => probeAlias(repo.host)}
                        className="rounded-md bg-raised-40 px-3 py-1.5 text-xs text-fg-3 hover:bg-subtle"
                      >
                        {m.repos.probeAlias}
                      </button>
                    </div>

                    <div className="space-y-1.5 pt-1">
                      <div className="flex items-center justify-between gap-3">
                        <div>
                          <p className="text-xs text-fg-3">
                            {m.repos.pinAlias}
                          </p>
                          <p className="text-[11px] text-fg-5">
                            {m.repos.pinAliasHint}
                          </p>
                        </div>
                        <Toggle
                          on={draft?.pinAlias ?? false}
                          onClick={() =>
                            setDrafts((prev) => ({
                              ...prev,
                              [repo.path]: {
                                ...(prev[repo.path] ?? {
                                  profileId: "",
                                  installHook: true,
                                  pinAlias: false,
                                }),
                                pinAlias: !(prev[repo.path]?.pinAlias ?? false),
                              },
                            }))
                          }
                        />
                      </div>
                      <div className="flex items-center justify-between gap-3">
                        <div>
                          <p className="text-xs text-fg-3">
                            {m.repos.installHook}
                          </p>
                          <p className="text-[11px] text-fg-5">
                            {m.repos.installHookHint}
                          </p>
                        </div>
                        <Toggle
                          on={draft?.installHook ?? true}
                          onClick={() =>
                            setDrafts((prev) => ({
                              ...prev,
                              [repo.path]: {
                                ...(prev[repo.path] ?? {
                                  profileId: "",
                                  pinAlias: false,
                                  installHook: true,
                                }),
                                installHook: !(
                                  prev[repo.path]?.installHook ?? true
                                ),
                              },
                            }))
                          }
                        />
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* Doctor */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="font-medium text-fg-2">{m.repos.doctorTitle}</h3>
              <p className="text-xs text-fg-5">{m.repos.doctorHint}</p>
            </div>
            <button
              onClick={() => run("doctor", reload)}
              disabled={busy === "doctor"}
              className="shrink-0 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-subtle disabled:opacity-50"
            >
              {busy === "doctor" ? m.repos.running : m.repos.run}
            </button>
          </div>

          {report && report.repos.length === 0 ? (
            <p className="text-xs text-fg-5">{m.repos.noBindings}</p>
          ) : report && report.repos.every((r) => r.ok) ? (
            <p className="text-xs text-success-fg">{m.repos.allOk}</p>
          ) : null}

          <ul className="space-y-3">
            {report?.repos
              .filter((r) => !r.ok)
              .map((repo) => (
                <li key={repo.path} className="rounded-md bg-raised p-3">
                  <div className="flex flex-wrap items-baseline justify-between gap-2">
                    <span className="text-sm font-medium text-fg-2">
                      {repo.name}
                    </span>
                    <span className="text-[11px] text-fg-5">
                      {repo.profile_name} · {repo.expected_email}
                    </span>
                  </div>
                  <ul className="mt-2 space-y-1">
                    {repo.checks.map((c) => (
                      <li
                        key={c.id}
                        className="flex items-baseline justify-between gap-3 text-[11px]"
                      >
                        <span
                          className={
                            c.ok
                              ? "text-fg-5"
                              : "text-red-600 dark:text-red-400"
                          }
                        >
                          {c.ok ? "✓" : "✗"} {checkLabel[c.id]}
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
                      onClick={() => fixRepo(repo.path)}
                      disabled={busy === `fix:${repo.path}`}
                      className="rounded-md bg-blue-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                    >
                      {m.repos.fix}
                    </button>
                    {repo.offending_emails.map((email) => (
                      <button
                        key={email}
                        onClick={() => allowEmail(repo.path, email)}
                        className="rounded-md bg-raised-40 px-3 py-1.5 text-xs text-fg-3 hover:bg-subtle"
                      >
                        {m.repos.allowEmail}: {email}
                      </button>
                    ))}
                  </div>
                </li>
              ))}
          </ul>
        </div>
      </div>

      {note && (
        <div className="border-t border-bd px-6 py-3">
          <p className="text-xs break-all whitespace-pre-wrap text-fg-3">
            {note}
          </p>
        </div>
      )}
    </div>
  );
}
