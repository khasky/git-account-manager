import { fmt, useI18n } from "../i18n";
import type { FolderWatch } from "../types";
import Spinner from "./Spinner";

interface Props {
  problems: FolderWatch[];
  busy: string;
  onRelink: (path: string, newPath: string) => void;
  onForget: (path: string) => void;
  onOpenProfile: (profileId: string) => void;
  onDismiss: () => void;
}

/** A folder stopped matching its rule while nobody was looking at this window.
 *  Shown over everything because the alternative is committing under the wrong
 *  account until someone happens to open the profile. */
export default function FolderAlert({
  problems,
  busy,
  onRelink,
  onForget,
  onOpenProfile,
  onDismiss,
}: Props) {
  const { m } = useI18n();
  if (problems.length === 0) return null;

  const stateLabel: Record<FolderWatch["state"], string> = {
    ok: "",
    missing: m.repos.watchMissing,
    "no-rule": m.repos.watchNoRule,
    "guard-off": m.repos.watchGuardOff,
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 p-6">
      <div className="max-h-full w-full max-w-lg overflow-y-auto rounded-lg border border-bd bg-dialog p-5 shadow-xl">
        <h2 className="text-base font-semibold text-fg">
          {m.repos.alertTitle}
        </h2>
        <p className="mt-1 text-xs text-fg-4">{m.repos.alertHint}</p>

        <ul className="mt-4 space-y-3">
          {problems.map((folder) => (
            <li key={folder.path} className="rounded-md bg-raised p-3">
              <code className="block truncate text-xs text-fg-2">
                {folder.path}
              </code>
              <p className="mt-1 text-[11px] text-danger-fg">
                {stateLabel[folder.state]}
              </p>
              <p className="text-[11px] text-fg-5">{folder.profile_name}</p>

              {folder.moved_to && (
                <p className="mt-1 text-[11px] text-fg-4">
                  {fmt(m.repos.movedFound, { path: folder.moved_to })}
                </p>
              )}

              <div className="mt-2 flex flex-wrap gap-2">
                {folder.moved_to && (
                  <button
                    type="button"
                    onClick={() =>
                      onRelink(folder.path, folder.moved_to as string)
                    }
                    disabled={busy !== ""}
                    className="inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1 text-[11px] font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                  >
                    {busy === `relink:${folder.path}` && <Spinner />}
                    {m.repos.relink}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => onOpenProfile(folder.profile_id)}
                  disabled={busy !== ""}
                  className="btn-raised-sm"
                >
                  {m.repos.openProfile}
                </button>
                {folder.state === "missing" && (
                  <button
                    type="button"
                    onClick={() => onForget(folder.path)}
                    disabled={busy !== ""}
                    className="inline-flex items-center gap-1.5 rounded-md px-3 py-1 text-[11px] text-fg-4 hover:text-red-500 disabled:opacity-50"
                  >
                    {busy === `forget:${folder.path}` && <Spinner />}
                    {m.repos.forgetFolder}
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>

        <div className="mt-4 flex justify-end">
          <button type="button" onClick={onDismiss} className="btn-raised-sm">
            {m.repos.alertDismiss}
          </button>
        </div>
      </div>
    </div>
  );
}
