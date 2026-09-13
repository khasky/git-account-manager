import { treeRow } from "../folderTree";
import { fmt, useI18n } from "../i18n";
import type { FolderRepo, PlatformId, RepoRoot } from "../types";
import InfoTip from "./InfoTip";

interface Props {
  root: RepoRoot;
  /** Only the platforms this profile actually connected. */
  platforms: PlatformId[];
  /** Every repository found under this folder, in path order. */
  repos: FolderRepo[];
  open: boolean;
  blocked: boolean;
  onToggleOpen: () => void;
  onRemove: () => void;
  onUpdate: (next: Partial<RepoRoot>) => void;
}

/** One watched folder: where it is, which account it hands out, and the whole
 *  hierarchy of repositories that will get it. Nothing here is chosen per
 *  repository — the rule is the folder, and the list says what that means. */
export default function RepoFolder({
  root,
  platforms,
  repos,
  open,
  blocked,
  onToggleOpen,
  onRemove,
  onUpdate,
}: Props) {
  const { m } = useI18n();
  const foreign = repos.filter((r) => r.foreign_host).length;

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
            disabled={blocked}
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
          disabled={blocked}
          className="rounded-md px-2 py-1 text-xs text-fg-4 hover:text-red-500 disabled:opacity-50"
        >
          {m.repos.remove}
        </button>
      </div>

      <button
        type="button"
        onClick={onToggleOpen}
        aria-expanded={open}
        className="flex w-full items-center gap-1 border-t border-bd pt-2 text-left text-[11px] text-fg-4 hover:text-fg-2"
      >
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
        <span>
          {fmt(m.repos.folderSummary, { total: repos.length })}
          {foreign > 0 && ` · ${fmt(m.repos.foreignCount, { foreign })}`}
        </span>
      </button>

      {open &&
        (repos.length === 0 ? (
          <p className="text-[11px] text-fg-5">{m.repos.folderEmpty}</p>
        ) : (
          <ul className="space-y-0.5">
            {repos.map((repo) => {
              const { depth, prefix, label } = treeRow(repo);
              return (
                <li
                  key={repo.path}
                  className="flex flex-wrap items-baseline gap-x-2 rounded px-1 py-0.5 text-[11px] hover:bg-raised-40"
                  style={{ paddingLeft: `${depth * 14 + 4}px` }}
                >
                  <span className="text-fg-6" aria-hidden="true">
                    {depth > 0 ? "└" : "•"}
                  </span>
                  <span className="text-fg-2">
                    {prefix && <span className="text-fg-6">{prefix}/</span>}
                    {label}
                  </span>
                  <code className="min-w-0 flex-1 truncate text-right text-fg-5">
                    {repo.full_name || m.repos.noRemote}
                  </code>
                  {repo.foreign_host && (
                    <span className="text-amber-600 dark:text-amber-400">
                      {m.repos.foreignHost}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        ))}

      {open && foreign > 0 && (
        <p className="flex items-start gap-1.5 text-[11px] text-amber-600 dark:text-amber-400">
          {m.repos.foreignHostHint}
          <InfoTip text={m.repos.foreignHostInfo} />
        </p>
      )}
    </li>
  );
}
