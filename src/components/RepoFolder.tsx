import { fmt, useI18n } from "../i18n";
import { decidedByEvidence } from "../repoEvidence";
import type { DiscoveredRepo, PlatformId, RepoRoot } from "../types";
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
  onToggleOpen: () => void;
  onRemove: () => void;
  onUpdate: (next: Partial<RepoRoot>) => void;
  onSelect: (path: string, checked: boolean) => void;
  onOverride: (path: string, next: Partial<DiscoveredRepo>) => void;
  onCheckAccess: (repo: DiscoveredRepo) => void;
  onProbeAlias: (host: string) => void;
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
  onToggleOpen,
  onRemove,
  onUpdate,
  onSelect,
  onOverride,
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
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-xs text-fg-3">{m.repos.installHook}</p>
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
            <p className="text-xs text-fg-3">{m.repos.pinAlias}</p>
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
                        onOverride(repo.path, {
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
                        onOverride(repo.path, {
                          pin_remote_alias: e.target.checked,
                        })
                      }
                      className="h-3 w-3 accent-blue-600"
                    />
                    {m.repos.aliasShort}
                  </label>
                  <button
                    type="button"
                    onClick={() => onCheckAccess(repo)}
                    disabled={busy === `access:${repo.path}`}
                    className="text-[11px] text-link hover:underline disabled:opacity-50"
                  >
                    {m.repos.verifyAccess}
                  </button>
                  <button
                    type="button"
                    onClick={() => onProbeAlias(repo.host)}
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
}
