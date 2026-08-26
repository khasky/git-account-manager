import { fmt, useI18n } from "../i18n";
import { decidedByEvidence } from "../repoEvidence";
import type { DiscoveredRepo, PlatformId, RepoNote, RepoRoot } from "../types";
import InfoTip from "./InfoTip";
import Spinner from "./Spinner";
import Toggle from "./Toggle";

interface Props {
  root: RepoRoot;
  profileId: string;
  /** Only the platforms this profile actually connected. */
  platforms: PlatformId[];
  /** The repositories found under this folder. */
  repos: DiscoveredRepo[];
  selected: Record<string, boolean>;
  open: boolean;
  busy: string;
  /** Some action is running: every other one waits its turn. */
  blocked: boolean;
  note: RepoNote | null;
  onToggleOpen: () => void;
  onRemove: () => void;
  onUpdate: (next: Partial<RepoRoot>) => void;
  onSelect: (path: string, checked: boolean) => void;
  onOverride: (path: string, next: Partial<DiscoveredRepo>) => void;
  onFollowFolder: (path: string) => void;
  onCheckAccess: (repo: DiscoveredRepo) => void;
  onProbeAlias: (repo: DiscoveredRepo) => void;
}

/** One watched folder: where it is, what its repositories inherit, and which of
 *  them this profile will claim on Save. */
export default function RepoFolder({
  root,
  profileId,
  platforms,
  repos,
  selected,
  open,
  busy,
  blocked,
  note,
  onToggleOpen,
  onRemove,
  onUpdate,
  onSelect,
  onOverride,
  onFollowFolder,
  onCheckAccess,
  onProbeAlias,
}: Props) {
  const { m } = useI18n();
  const chosen = repos.filter((r) => selected[r.path]).length;
  const pending = repos.filter(
    (r) => !decidedByEvidence(r, profileId) && !r.bound,
  ).length;

  const reasonLabel: Record<DiscoveredRepo["reason"], string> = {
    alias: m.repos.reasonAlias,
    owner: m.repos.reasonOwner,
    ambiguous: m.repos.reasonAmbiguous,
    unknown: m.repos.reasonUnknown,
  };

  return (
    <li className="space-y-2 rounded-md bg-raised p-3">
      <div className="flex flex-wrap items-center gap-2">
        <code className="flex-1 truncate text-xs text-fg-3">{root.path}</code>
        {platforms.length > 1 && (
          <select
            aria-label={m.repos.sectionTitle}
            value={root.platform}
            onChange={(e) =>
              onUpdate({ platform: e.target.value as PlatformId })
            }
            className="select-sm"
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
          onClick={onRemove}
          className="rounded-md px-2 py-1 text-xs text-fg-4 hover:text-red-500"
        >
          {m.repos.remove}
        </button>
      </div>

      <div className="space-y-1.5 border-t border-bd pt-2">
        <p className="text-[11px] text-fg-5">{m.repos.folderDefaults}</p>
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="flex items-center gap-1.5 text-xs text-fg-3">
              {m.repos.installHook}
              <InfoTip text={m.repos.installHookInfo} />
            </p>
            <p className="text-[11px] text-fg-5">{m.repos.installHookHint}</p>
          </div>
          <Toggle
            size="sm"
            on={root.install_hook}
            onClick={() => onUpdate({ install_hook: !root.install_hook })}
          />
        </div>
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="flex items-center gap-1.5 text-xs text-fg-3">
              {m.repos.pinAlias}
              <InfoTip text={m.repos.pinAliasInfo} />
            </p>
            <p className="text-[11px] text-fg-5">{m.repos.pinAliasHint}</p>
          </div>
          <Toggle
            size="sm"
            on={root.pin_remote_alias}
            onClick={() =>
              onUpdate({ pin_remote_alias: !root.pin_remote_alias })
            }
          />
        </div>
      </div>

      <button
        type="button"
        onClick={onToggleOpen}
        aria-expanded={open}
        className="flex w-full items-center gap-1 border-t border-bd pt-2 text-left text-[11px] text-fg-4 hover:text-fg-2"
      >
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
        <span>
          {fmt(m.repos.folderSummary, { total: repos.length, chosen })}
          {pending > 0 && ` · ${fmt(m.repos.pending, { pending })}`}
        </span>
      </button>

      {open && repos.length > 0 && (
        <ul className="space-y-2">
          {repos.map((repo) => {
            const needsDecision =
              !decidedByEvidence(repo, profileId) && !repo.bound;
            return (
              <li
                key={repo.path}
                className={`space-y-1.5 rounded-md bg-raised-40 p-2 ${
                  needsDecision ? "ring-1 ring-amber-500/40" : ""
                }`}
              >
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    type="checkbox"
                    id={`pick-${repo.path}`}
                    checked={selected[repo.path] ?? false}
                    onChange={(e) => onSelect(repo.path, e.target.checked)}
                    className="h-3.5 w-3.5 accent-blue-600"
                  />
                  <label
                    htmlFor={`pick-${repo.path}`}
                    className="text-xs font-medium text-fg-2"
                  >
                    {repo.name}
                  </label>
                </div>
                <p className="text-[11px] text-fg-5">
                  {reasonLabel[repo.reason]}
                </p>
                <code className="block truncate text-[11px] text-fg-4">
                  {repo.remote_url}
                </code>

                {/* The same two settings as the folder above, shown here so it
                    is visible which ones this repository will actually get and
                    whether they still come from the folder. */}
                <div className="space-y-1 rounded bg-raised/60 px-2 py-1.5">
                  <div className="flex items-center justify-between gap-2">
                    <span className="flex items-center gap-1.5 text-[11px] text-fg-5">
                      {repo.overrides_root
                        ? m.repos.setApart
                        : m.repos.followsFolder}
                      <InfoTip text={m.repos.overrideInfo} />
                    </span>
                    {repo.overrides_root && (
                      <button
                        type="button"
                        onClick={() => onFollowFolder(repo.path)}
                        className="text-[11px] text-link hover:underline"
                      >
                        {m.repos.useFolderDefaults}
                      </button>
                    )}
                  </div>
                  <label className="flex items-center gap-1.5 text-[11px] text-fg-4">
                    <input
                      type="checkbox"
                      checked={repo.install_hook}
                      onChange={(e) =>
                        onOverride(repo.path, {
                          install_hook: e.target.checked,
                        })
                      }
                      className="h-3 w-3 accent-blue-600"
                    />
                    {m.repos.installHook}
                  </label>
                  <label className="flex items-center gap-1.5 text-[11px] text-fg-4">
                    <input
                      type="checkbox"
                      checked={repo.pin_remote_alias}
                      onChange={(e) =>
                        onOverride(repo.path, {
                          pin_remote_alias: e.target.checked,
                        })
                      }
                      className="h-3 w-3 accent-blue-600"
                    />
                    {m.repos.pinAlias}
                  </label>
                </div>

                <div className="flex flex-wrap items-center gap-3 pt-0.5">
                  <button
                    type="button"
                    onClick={() => onCheckAccess(repo)}
                    disabled={blocked}
                    className="inline-flex items-center gap-1 text-[11px] text-link hover:underline disabled:opacity-50"
                  >
                    {busy === `access:${repo.path}` && <Spinner />}
                    {m.repos.verifyAccess}
                  </button>
                  <button
                    type="button"
                    onClick={() => onProbeAlias(repo)}
                    disabled={blocked}
                    className="inline-flex items-center gap-1 text-[11px] text-link hover:underline disabled:opacity-50"
                  >
                    {busy === `ssh:${repo.path}` && <Spinner />}
                    {m.repos.probeAlias}
                  </button>
                </div>

                {note &&
                  (note.key === `access:${repo.path}` ||
                    note.key === `ssh:${repo.path}`) && (
                    <p
                      className={`rounded bg-raised px-2 py-1.5 text-[11px] break-all whitespace-pre-wrap ${
                        note.tone === "bad" ? "text-danger-fg" : "text-fg-3"
                      }`}
                    >
                      {note.text}
                    </p>
                  )}
              </li>
            );
          })}
        </ul>
      )}
    </li>
  );
}
