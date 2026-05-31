import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { OAuthSettings, OpenSshIntegrationProbe } from "../types";
import { useTheme } from "../ThemeContext";
import { useI18n, fmt, rich, LANGUAGES, type LangCode } from "../i18n";

interface Props {
  onBack: () => void;
}

function formatInvokeError(e: unknown, fallback: string): string {
  if (typeof e === "string") return e;
  if (
    e &&
    typeof e === "object" &&
    "message" in e &&
    typeof (e as { message: unknown }).message === "string"
  ) {
    return (e as { message: string }).message;
  }
  return fallback;
}

const SunIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
    />
  </svg>
);

const MoonIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
    />
  </svg>
);

const MonitorIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
    />
  </svg>
);

export default function SettingsPage({ onBack }: Props) {
  const [githubId, setGithubId] = useState("");
  const [gitlabId, setGitlabId] = useState("");
  const [useOpenSsh, setUseOpenSsh] = useState(false);
  const [openSshProbe, setOpenSshProbe] = useState<OpenSshIntegrationProbe | null>(
    null,
  );
  const [autostart, setAutostart] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const { preference, setPreference } = useTheme();
  const { m, lang, setLang } = useI18n();

  useEffect(() => {
    invoke<OAuthSettings>("get_settings")
      .then((s) => {
        setGithubId(s.github_client_id);
        setGitlabId(s.gitlab_client_id);
        setUseOpenSsh(Boolean(s.use_openssh_for_git_tools));
      })
      .catch(() => {});
    invoke<OpenSshIntegrationProbe>("openssh_integration_probe")
      .then(setOpenSshProbe)
      .catch(() => setOpenSshProbe({ available: false, sshExe: null }));
    isAutostartEnabled()
      .then(setAutostart)
      .catch(() => {});
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onBack]);

  async function toggleAutostart() {
    try {
      if (autostart) {
        await disableAutostart();
        setAutostart(false);
      } else {
        await enableAutostart();
        setAutostart(true);
      }
    } catch {
      /* ignore */
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaveError("");
    try {
      await invoke("save_settings", {
        settings: {
          github_client_id: githubId.trim(),
          gitlab_client_id: gitlabId.trim(),
          use_openssh_for_git_tools: useOpenSsh,
        },
      });
      const probe = await invoke<OpenSshIntegrationProbe>(
        "openssh_integration_probe",
      );
      setOpenSshProbe(probe);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setSaveError(formatInvokeError(e, m.settings.saveError));
    } finally {
      setSaving(false);
    }
  }

  const themeOptions = [
    { value: "light" as const, label: m.theme.light, icon: <SunIcon /> },
    { value: "dark" as const, label: m.theme.dark, icon: <MoonIcon /> },
    { value: "system" as const, label: m.theme.system, icon: <MonitorIcon /> },
  ];

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
        <h2 className="text-lg font-semibold text-fg">{m.settings.title}</h2>
      </div>

      <div className="flex-1 space-y-6 overflow-y-auto p-6">
        {/* General */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <h3 className="font-medium text-fg-2">{m.settings.general}</h3>

          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-fg-3">{m.settings.theme}</p>
              <p className="text-xs text-fg-5">{m.settings.themeHint}</p>
            </div>
            <div className="flex rounded-lg border border-bd">
              {themeOptions.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setPreference(opt.value)}
                  className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium transition-colors first:rounded-l-[7px] last:rounded-r-[7px] ${
                    preference === opt.value
                      ? "bg-selected-bg text-selected-fg"
                      : "text-fg-4 hover:bg-raised hover:text-fg-2"
                  }`}
                  title={opt.label}
                >
                  {opt.icon}
                  <span className="hidden sm:inline">{opt.label}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-fg-3">{m.settings.autostart}</p>
              <p className="text-xs text-fg-5">{m.settings.autostartHint}</p>
            </div>
            <button
              onClick={toggleAutostart}
              className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
                autostart ? "bg-emerald-600" : "bg-toggle-off"
              }`}
            >
              <span
                className={`inline-block h-5 w-5 rounded-full bg-white shadow transition-transform ${
                  autostart ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-fg-3">{m.settings.language}</p>
              <p className="text-xs text-fg-5">{m.settings.languageHint}</p>
            </div>
            <select
              value={lang}
              onChange={(e) => setLang(e.target.value as LangCode)}
              className="rounded-md border border-bd-s bg-input px-3 py-1.5 text-sm text-fg outline-none focus:border-blue-500"
            >
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>
                  {l.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* TortoiseGit + Git: OpenSSH (Windows) */}
        {openSshProbe?.available ? (
          <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
            <h3 className="font-medium text-fg-2">{m.settings.tortoise.title}</h3>
            <p className="text-xs text-fg-4 leading-relaxed">
              {rich(m.settings.tortoise.intro)}
            </p>
            {openSshProbe.sshExe ? (
              <p className="text-xs text-fg-5">
                {rich(
                  fmt(m.settings.tortoise.detected, {
                    path: openSshProbe.sshExe,
                  }),
                  { codeClass: "break-all text-fg-3" },
                )}
              </p>
            ) : (
              <p className="text-xs text-amber-600 dark:text-amber-400">
                {rich(m.settings.tortoise.notFound, {
                  href: "https://git-scm.com/download/win",
                })}
              </p>
            )}
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm text-fg-3">{m.settings.tortoise.toggle}</p>
                <p className="text-xs text-fg-5">
                  {rich(m.settings.tortoise.toggleHint, {
                    codeClass: "text-fg-4",
                  })}
                </p>
              </div>
              <button
                type="button"
                onClick={() => setUseOpenSsh(!useOpenSsh)}
                className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
                  useOpenSsh ? "bg-emerald-600" : "bg-toggle-off"
                }`}
                aria-pressed={useOpenSsh}
              >
                <span
                  className={`inline-block h-5 w-5 rounded-full bg-white shadow transition-transform ${
                    useOpenSsh ? "translate-x-5" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
            {useOpenSsh && !openSshProbe.sshExe ? (
              <p className="text-xs text-red-600 dark:text-red-400">
                {rich(m.settings.tortoise.willFail)}
              </p>
            ) : null}
          </div>
        ) : null}

        {/* GitHub OAuth */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <h3 className="font-medium text-fg-2">{m.settings.github.title}</h3>
          <p className="text-xs text-fg-4">{m.settings.github.required}</p>
          <ol className="list-inside list-decimal space-y-1 text-xs text-fg-4">
            <li>
              {rich(m.settings.github.step1, {
                onLink: () => openUrl("https://github.com/settings/developers"),
              })}
            </li>
            <li>{rich(m.settings.github.step2)}</li>
            <li>{rich(m.settings.github.step3)}</li>
            <li>{rich(m.settings.github.step4)}</li>
            <li>{rich(m.settings.github.step5)}</li>
            <li>{rich(m.settings.github.step6)}</li>
          </ol>
          <input
            type="text"
            value={githubId}
            onChange={(e) => setGithubId(e.target.value)}
            placeholder={m.settings.github.placeholder}
            className="w-full rounded-md border border-bd-s bg-input px-3 py-2 text-sm text-fg outline-none focus:border-blue-500"
          />
        </div>

        {/* GitLab OAuth */}
        <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
          <h3 className="font-medium text-fg-2">{m.settings.gitlab.title}</h3>
          <p className="text-xs text-fg-4">{m.settings.gitlab.required}</p>
          <ol className="list-inside list-decimal space-y-1 text-xs text-fg-4">
            <li>
              {rich(m.settings.gitlab.step1, {
                onLink: () =>
                  openUrl("https://gitlab.com/-/user_settings/applications"),
              })}
            </li>
            <li>{rich(m.settings.gitlab.step2)}</li>
            <li>{rich(m.settings.gitlab.step3)}</li>
            <li>{rich(m.settings.gitlab.step4)}</li>
            <li>{rich(m.settings.gitlab.step5)}</li>
            <li>{rich(m.settings.gitlab.step6)}</li>
          </ol>
          <input
            type="text"
            value={gitlabId}
            onChange={(e) => setGitlabId(e.target.value)}
            placeholder={m.settings.gitlab.placeholder}
            className="w-full rounded-md border border-bd-s bg-input px-3 py-2 text-sm text-fg outline-none focus:border-blue-500"
          />
        </div>
      </div>

      <div className="flex items-center gap-3 border-t border-bd px-6 py-4">
        <button
          onClick={handleSave}
          disabled={saving}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
        >
          {saving ? m.settings.saving : m.settings.save}
        </button>
        {saved && <span className="text-sm text-success-fg">{m.settings.saved}</span>}
        {saveError ? (
          <span className="max-w-md text-sm text-red-600 dark:text-red-400">
            {saveError}
          </span>
        ) : null}
      </div>
    </div>
  );
}
