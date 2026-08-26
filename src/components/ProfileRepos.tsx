import { open } from "@tauri-apps/plugin-dialog";
import {
  type Dispatch,
  type SetStateAction,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import * as api from "../api";
import { fmt, useI18n } from "../i18n";
import { decidedByEvidence } from "../repoEvidence";
import type {
  DiscoveredRepo,
  PlatformId,
  Profile,
  RepoNote,
  RepoRoot,
  RepoStatus,
} from "../types";
import InfoTip from "./InfoTip";
import RepoDoctor from "./RepoDoctor";
import RepoFolder from "./RepoFolder";
import Spinner from "./Spinner";

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
  /** The first read of state and doctor is still in flight. */
  loading: boolean;
  onFixed: () => void;
}

/** The folders this profile owns: adding them, scanning them, and choosing
 *  which of their repositories it claims. Nothing here writes to disk — Save
 *  applies the whole set, which is what lets a profile that does not exist yet
 *  be configured. */
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
  loading,
  onFixed,
}: Props) {
  const { m } = useI18n();
  const [busy, setBusy] = useState("");
  // Addressed to the control that produced it: a result printed at the bottom of
  // a long panel is a result the user who clicked never sees.
  const [note, setNote] = useState<RepoNote | null>(null);
  const [openRoots, setOpenRoots] = useState<Record<string, boolean>>({});
  const autoScanned = useRef(false);

  const run = useCallback(
    async <T,>(key: string, task: () => Promise<T>): Promise<T | null> => {
      setBusy(key);
      setNote(null);
      try {
        return await task();
      } catch (e) {
        setNote({
          key,
          tone: "bad",
          text: fmt(m.repos.error, { error: String(e) }),
        });
        return null;
      } finally {
        setBusy("");
      }
    },
    [m.repos.error],
  );

  const scanWith = useCallback(
    async (next: RepoRoot[]) => {
      const found = await run("scan", () =>
        api.scanProfileRepositories({ profile, roots: next }),
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
    },
    [profile, run, setRepos, setSelected],
  );

  // Opening a profile that already has folders must show what is in them.
  // Without this the summary reads "0 repositories" over a folder holding a
  // dozen bound ones, which is not a neutral empty state — it is wrong.
  // Deliberately once per mount: re-running on every roots edit would walk the
  // disk on each toggle.
  // biome-ignore lint/correctness/useExhaustiveDependencies: one-shot mount scan
  useEffect(() => {
    if (autoScanned.current || roots.length === 0) return;
    autoScanned.current = true;
    scanWith(roots);
  }, [roots]);

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
        platform: (profile.default_platform ??
          platforms[0] ??
          "github") as PlatformId,
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

  /** Hands a repository back to its folder, so a later change to the folder's
   *  defaults reaches it again. */
  function followFolder(path: string) {
    setRepos((prev) =>
      prev.map((r) => {
        if (r.path !== path) return r;
        const root = roots.find((x) => x.path === r.root_path);
        return {
          ...r,
          install_hook: root?.install_hook ?? r.install_hook,
          pin_remote_alias: root?.pin_remote_alias ?? r.pin_remote_alias,
          overrides_root: false,
        };
      }),
    );
  }

  async function checkAccess(repo: DiscoveredRepo) {
    const platform =
      repo.suggested_platform ?? (platforms.length === 1 ? platforms[0] : null);
    if (!platform) return;
    const access = await run(`access:${repo.path}`, () =>
      api.verifyRepoAccess({
        profileId: profile.id,
        platform,
        owner: repo.owner,
        repo: repo.repo,
      }),
    );
    if (!access) return;
    setNote({
      key: `access:${repo.path}`,
      tone: access.reachable ? "ok" : "bad",
      text: access.reachable
        ? fmt(m.repos.accessOk, { full: access.full_name })
        : fmt(m.repos.accessDenied, {
            full: access.full_name,
            detail: access.detail,
          }),
    });
  }

  /** Keyed by the repository rather than the host so the answer appears under
   *  the row that was clicked, even when several share one alias. */
  async function probeAlias(repo: DiscoveredRepo) {
    const key = `ssh:${repo.path}`;
    const answer = await run(key, () => api.probeSshAlias(repo.host));
    if (answer) setNote({ key, tone: "ok", text: answer });
  }

  async function fixRepo(path: string) {
    const done = await run(`fix:${path}`, () => api.fixRepository(path));
    if (done === null) return;
    setNote({ key: `fix:${path}`, tone: "ok", text: m.repos.fixed });
    onFixed();
  }

  async function allowEmail(path: string, email: string) {
    const key = `allow:${path}`;
    const done = await run(key, () =>
      api.allowEmailInRepository({ path, email }),
    );
    if (done === null) return;
    setNote({ key, tone: "ok", text: fmt(m.repos.allowed, { email }) });
    onFixed();
  }

  // One action at a time: these write to Git repositories, and a second click
  // while the first is still running would race it over the same files.
  const blocked = busy !== "" || loading;

  return (
    <div className="panel space-y-3">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h4 className="flex items-center gap-1.5 font-medium text-fg-2">
            {m.repos.sectionTitle}
            <InfoTip text={m.repos.sectionInfo} />
          </h4>
          <p className="text-xs text-fg-5">{m.repos.sectionHint}</p>
        </div>
        <div className="flex shrink-0 gap-2">
          {roots.length > 0 && (
            <button
              type="button"
              onClick={() => scanWith(roots)}
              disabled={blocked}
              className="btn-raised-sm inline-flex items-center gap-1.5"
            >
              {busy === "scan" && <Spinner />}
              {busy === "scan" ? m.repos.scanning : m.repos.rescan}
            </button>
          )}
          <button
            type="button"
            onClick={addFolder}
            disabled={blocked}
            className="btn-raised-sm"
          >
            {m.repos.addFolder}
          </button>
        </div>
      </div>

      {loading ? (
        <p className="flex items-center gap-2 text-xs text-fg-5">
          <Spinner />
          {m.repos.loading}
        </p>
      ) : roots.length === 0 ? (
        <p className="text-xs text-fg-5">{m.repos.noRoots}</p>
      ) : (
        <ul className="space-y-3">
          {roots.map((root) => (
            <RepoFolder
              key={root.path}
              root={root}
              profileId={profile.id}
              platforms={platforms}
              repos={repos.filter((r) => r.root_path === root.path)}
              selected={selected}
              open={openRoots[root.path] ?? false}
              busy={busy}
              blocked={blocked}
              note={note}
              onToggleOpen={() =>
                setOpenRoots((prev) => ({
                  ...prev,
                  [root.path]: !(prev[root.path] ?? false),
                }))
              }
              onRemove={() => removeFolder(root.path)}
              onUpdate={(next) => updateFolder(root.path, next)}
              onSelect={(path, checked) =>
                setSelected((prev) => ({ ...prev, [path]: checked }))
              }
              onOverride={overrideRepo}
              onFollowFolder={followFolder}
              onCheckAccess={checkAccess}
              onProbeAlias={probeAlias}
            />
          ))}
        </ul>
      )}

      <RepoDoctor
        problems={statuses.filter((s) => !s.ok)}
        busy={busy}
        blocked={blocked}
        note={note}
        onFix={fixRepo}
        onAllowEmail={allowEmail}
      />

      {/* Anything not addressed to a row — a failure raised before one was
          identified — still has to reach the user somewhere. */}
      {note && !note.key.includes(":") && (
        <p
          className={`border-t border-bd pt-2 text-[11px] break-all whitespace-pre-wrap ${
            note.tone === "bad" ? "text-danger-fg" : "text-fg-3"
          }`}
        >
          {note.text}
        </p>
      )}
    </div>
  );
}
