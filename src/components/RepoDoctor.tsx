import { useI18n } from "../i18n";
import type { RepoCheck, RepoStatus } from "../types";

interface Props {
  /** Only the rows that failed; a healthy repository has nothing to show. */
  problems: RepoStatus[];
  busy: string;
  onFix: (path: string) => void;
  onAllowEmail: (path: string, email: string) => void;
}

/** What drifted in this profile's repositories, and the two ways to settle it:
 *  rewrite the binding, or accept an address the history check flagged. */
export default function RepoDoctor({
  problems,
  busy,
  onFix,
  onAllowEmail,
}: Props) {
  const { m } = useI18n();

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

  if (problems.length === 0) return null;

  return (
    <div className="space-y-2 border-t border-bd pt-3">
      <div>
        <h4 className="text-xs font-medium text-fg-2">{m.repos.doctorTitle}</h4>
        <p className="text-[11px] text-fg-5">{m.repos.doctorHint}</p>
      </div>
      <ul className="space-y-2">
        {problems.map((repo) => (
          <li key={repo.path} className="rounded-md bg-raised p-3">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <span className="text-xs font-medium text-fg-2">{repo.name}</span>
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
                      <span aria-hidden="true">✗</span> {checkLabel[c.id]}
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
                onClick={() => onFix(repo.path)}
                disabled={busy === `fix:${repo.path}`}
                className="rounded-md bg-blue-600 px-3 py-1 text-[11px] font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
              >
                {m.repos.fix}
              </button>
              {repo.offending_emails.map((email) => (
                <button
                  type="button"
                  key={email}
                  onClick={() => onAllowEmail(repo.path, email)}
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
  );
}
