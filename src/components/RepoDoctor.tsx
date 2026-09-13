import { fmt, useI18n } from "../i18n";
import type { FolderCheck, FolderStatus, RepoNote } from "../types";
import InfoTip from "./InfoTip";
import Spinner from "./Spinner";

interface Props {
  /** Only the folders that failed; a healthy one has nothing to show. */
  problems: FolderStatus[];
  busy: string;
  /** Some action is running: every other one waits its turn. */
  blocked: boolean;
  note: RepoNote | null;
  onFix: (path: string) => void;
  onRelink: (path: string, newPath: string) => void;
  onForget: (path: string) => void;
}

/** What stopped holding in this profile's folders, and the way to settle it:
 *  rewrite the rule and take back what overrides it, follow the folder to
 *  where it moved, or let it go. */
export default function RepoDoctor({
  problems,
  busy,
  blocked,
  note,
  onFix,
  onRelink,
  onForget,
}: Props) {
  const { m } = useI18n();

  const checkLabel: Record<FolderCheck["id"], string> = {
    exists: m.repos.checkExists,
    rule: m.repos.checkRule,
    identity: m.repos.checkIdentity,
    local: m.repos.checkLocal,
    guard: m.repos.checkHooks,
  };

  if (problems.length === 0) return null;

  return (
    <div className="space-y-2 border-t border-bd pt-3">
      <div>
        <h4 className="flex items-center gap-1.5 text-xs font-medium text-fg-2">
          {m.repos.doctorTitle}
          <InfoTip text={m.repos.doctorInfo} />
        </h4>
        <p className="text-[11px] text-fg-5">{m.repos.doctorHint}</p>
      </div>
      <ul className="space-y-2">
        {problems.map((folder) => {
          const missing = folder.checks.some((c) => c.id === "exists" && !c.ok);
          return (
            <li key={folder.path} className="rounded-md bg-raised p-3">
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <code className="text-xs font-medium text-fg-2">
                  {folder.path}
                </code>
                <span className="text-[11px] text-fg-5">
                  {folder.expected_email}
                </span>
              </div>

              <ul className="mt-2 space-y-1">
                {folder.checks
                  .filter((c) => !c.ok)
                  .map((c) => (
                    <li
                      key={c.id}
                      className="flex items-baseline justify-between gap-3 text-[11px]"
                    >
                      <span className="text-red-600 dark:text-red-400">
                        <span aria-hidden="true">✗</span> {checkLabel[c.id]}
                      </span>
                      <span className="min-w-0 truncate text-right text-fg-4">
                        {c.detail}
                      </span>
                    </li>
                  ))}
              </ul>

              {folder.overrides.length > 0 && (
                <ul className="mt-2 space-y-0.5 border-t border-bd pt-2">
                  {folder.overrides.map((repo) => (
                    <li
                      key={repo.path}
                      className="flex items-baseline justify-between gap-3 text-[11px]"
                    >
                      <span className="text-fg-3">{repo.relative}</span>
                      <code className="min-w-0 truncate text-right text-fg-5">
                        {repo.detail}
                      </code>
                    </li>
                  ))}
                </ul>
              )}

              {folder.bypassed.length > 0 && (
                <p className="mt-2 flex items-start gap-1.5 text-[11px] text-amber-600 dark:text-amber-400">
                  {fmt(m.repos.bypassedCount, {
                    count: folder.bypassed.length,
                  })}
                  <InfoTip text={m.repos.bypassedInfo} />
                </p>
              )}

              <div className="mt-2 flex flex-wrap items-center gap-2">
                {missing ? (
                  <>
                    {folder.moved_to && (
                      <button
                        type="button"
                        onClick={() =>
                          onRelink(folder.path, folder.moved_to as string)
                        }
                        disabled={blocked}
                        className="inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1 text-[11px] font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                      >
                        {busy === `relink:${folder.path}` && <Spinner />}
                        {fmt(m.repos.relinkTo, { path: folder.moved_to })}
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => onForget(folder.path)}
                      disabled={blocked}
                      className="inline-flex items-center gap-1.5 rounded-md bg-raised-40 px-3 py-1 text-[11px] text-fg-3 hover:bg-subtle disabled:opacity-50"
                    >
                      {busy === `forget:${folder.path}` && <Spinner />}
                      {m.repos.forgetFolder}
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    onClick={() => onFix(folder.path)}
                    disabled={blocked}
                    className="inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1 text-[11px] font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
                  >
                    {busy === `fix:${folder.path}` && <Spinner />}
                    {m.repos.fix}
                  </button>
                )}
              </div>

              {missing && !folder.moved_to && (
                <p className="mt-2 text-[11px] text-fg-5">
                  {m.repos.movedUnknown}
                </p>
              )}

              {note?.key.endsWith(`:${folder.path}`) && (
                <p
                  className={`mt-2 text-[11px] break-all whitespace-pre-wrap ${
                    note.tone === "bad" ? "text-danger-fg" : "text-success-fg"
                  }`}
                >
                  {note.text}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
