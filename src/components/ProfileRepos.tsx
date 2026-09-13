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
import type {
  FolderRepo,
  FolderStatus,
  PlatformId,
  Profile,
  RepoNote,
  RepoRoot,
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
  repos: FolderRepo[];
  setRepos: Dispatch<SetStateAction<FolderRepo[]>>;
  /** Doctor rows belonging to this profile. */
  statuses: FolderStatus[];
  /** The first read of state and doctor is still in flight. */
  loading: boolean;
  onFixed: () => void;
}

/** The folders this profile owns. Each one is a rule: everything under it, at
 *  any depth, gets this account's identity and key from a generated
 *  `includeIf` block, so nothing is written into a repository and a clone
 *  landing there tomorrow is covered without being asked about. Nothing is
 *  applied until Save, which is what lets a profile that does not exist yet be
 *  configured. */
export default function ProfileRepos({
  profile,
  platforms,
  roots,
  setRoots,
  repos,
  setRepos,
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
        api.scanProfileFolders({ profile, roots: next }),
      );
      if (!found) return;
      setRepos(found);
      setOpenRoots(Object.fromEntries(next.map((r) => [r.path, true])));
    },
    [profile, run, setRepos],
  );

  // Opening a profile that already has folders must show what is in them.
  // Without this the summary reads "0 repositories" over a folder holding a
  // dozen, which is not a neutral empty state — it is wrong. Once per mount:
  // re-running on every roots edit would walk the disk on each keystroke.
  // biome-ignore lint/correctness/useExhaustiveDependencies: one-shot mount scan
  useEffect(() => {
    if (autoScanned.current || roots.length === 0) return;
    autoScanned.current = true;
    scanWith(roots);
  }, [roots]);

  async function addFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    const path = picked.replace(/\\/g, "/").replace(/\/+$/, "");
    if (roots.some((r) => r.path === path)) return;
    const next: RepoRoot[] = [
      ...roots,
      {
        path,
        profile_id: profile.id,
        platform: (profile.default_platform ??
          platforms[0] ??
          "github") as PlatformId,
        fingerprint: [],
      },
    ];
    setRoots(next);
    await scanWith(next);
  }

  function removeFolder(path: string) {
    setRoots((prev) => prev.filter((r) => r.path !== path));
    setRepos((prev) => prev.filter((r) => r.root_path !== path));
  }

  function updateFolder(path: string, next: Partial<RepoRoot>) {
    setRoots((prev) =>
      prev.map((r) => (r.path === path ? { ...r, ...next } : r)),
    );
  }

  async function fixFolder(path: string) {
    const cleaned = await run(`fix:${path}`, () => api.fixFolder(path));
    if (cleaned === null) return;
    setNote({
      key: `fix:${path}`,
      tone: "ok",
      text: fmt(m.repos.fixed, { count: cleaned }),
    });
    onFixed();
  }

  async function relinkFolder(path: string, newPath: string) {
    const done = await run(`relink:${path}`, () =>
      api.relinkFolder({ path, newPath }),
    );
    if (done === null) return;
    setRoots((prev) =>
      prev.map((r) => (r.path === path ? { ...r, path: newPath } : r)),
    );
    onFixed();
  }

  async function forgetFolder(path: string) {
    const done = await run(`forget:${path}`, () => api.forgetFolder(path));
    if (done === null) return;
    removeFolder(path);
    onFixed();
  }

  // One action at a time: these rewrite the machine's Git config, and a second
  // click while the first is still running would race it over the same file.
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
              platforms={platforms}
              repos={repos.filter((r) => r.root_path === root.path)}
              open={openRoots[root.path] ?? false}
              blocked={blocked}
              onToggleOpen={() =>
                setOpenRoots((prev) => ({
                  ...prev,
                  [root.path]: !(prev[root.path] ?? false),
                }))
              }
              onRemove={() => removeFolder(root.path)}
              onUpdate={(next) => updateFolder(root.path, next)}
            />
          ))}
        </ul>
      )}

      {/* The report describes the folders as saved; one removed in this draft
          still has its rule until Save, so its rows are hidden here rather
          than shown as problems the user has already dealt with. */}
      <RepoDoctor
        problems={statuses.filter(
          (s) => !s.ok && roots.some((r) => r.path === s.path),
        )}
        busy={busy}
        blocked={blocked}
        note={note}
        onFix={fixFolder}
        onRelink={relinkFolder}
        onForget={forgetFolder}
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
