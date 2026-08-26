import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import * as api from "../api";
import { fmt, LANGUAGES, type LangCode, rich, useI18n } from "../i18n";
import { useTheme } from "../ThemeContext";
import type {
  GuardSettings,
  GuardStatus,
  OpenSshIntegrationProbe,
} from "../types";
import { BackIcon, MonitorIcon, MoonIcon, SunIcon } from "./icons";
import Toggle from "./Toggle";

interface Props {
  onBack: () => void;
}

const SAVED_HINT_MS = 2000;

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

export default function SettingsPage({ onBack }: Props) {
  const [githubId, setGithubId] = useState("");
  const [gitlabId, setGitlabId] = useState("");
  const [useOpenSsh, setUseOpenSsh] = useState(false);
  const [openSshProbe, setOpenSshProbe] =
    useState<OpenSshIntegrationProbe | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [guard, setGuard] = useState<GuardSettings | null>(null);
  // What the guard rails actually did to this machine, read back rather than
  // assumed from the switches.
  const [guardStatus, setGuardStatus] = useState<GuardStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const { preference, setPreference } = useTheme();
  const { m, lang, setLang } = useI18n();

  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setGithubId(s.github_client_id);
        setGitlabId(s.gitlab_client_id);
        setUseOpenSsh(Boolean(s.use_openssh_for_git_tools));
      })
      .catch(() => {});
    api
      .openSshIntegrationProbe()
      .then(setOpenSshProbe)
      .catch(() => setOpenSshProbe({ available: false, ssh_exe: null }));
    isAutostartEnabled()
      .then(setAutostart)
      .catch(() => {});
    api
      .getRepoState()
      .then((s) => setGuard(s.guard))
      .catch(() => {});
    api
      .doctor()
      .then((r) => setGuardStatus(r.guard))
      .catch(() => {});
  }, []);

  // The "Saved" hint is on a timer, and leaving the page before it fires would
  // otherwise set state on a component that is gone.
  const savedHintRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (savedHintRef.current) clearTimeout(savedHintRef.current);
    };
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

  // The guard rails touch the machine's Git config, so each one applies the
  // moment it is toggled rather than waiting for Save — the same as autostart.
  async function toggleGuard(key: keyof GuardSettings) {
    if (!guard) return;
    const next = { ...guard, [key]: !guard[key] };
    setGuard(next);
    try {
      await api.saveGuardSettings(next);
      setSaveError("");
    } catch (e) {
      setGuard(guard);
      setSaveError(formatInvokeError(e, m.settings.saveError));
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaveError("");
    try {
      await api.saveSettings({
        github_client_id: githubId.trim(),
        gitlab_client_id: gitlabId.trim(),
        use_openssh_for_git_tools: useOpenSsh,
      });
      setOpenSshProbe(await api.openSshIntegrationProbe());
      setSaved(true);
      if (savedHintRef.current) clearTimeout(savedHintRef.current);
      savedHintRef.current = setTimeout(() => {
        setSaved(false);
        savedHintRef.current = null;
      }, SAVED_HINT_MS);
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
        <button
          type="button"
          onClick={onBack}
          title={m.settings.title}
          className="text-fg-4 hover:text-fg-2"
        >
          <BackIcon />
        </button>
        <h2 className="text-lg font-semibold text-fg">{m.settings.title}</h2>
      </div>

      <div className="flex-1 space-y-6 overflow-y-auto p-6">
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
                  type="button"
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
            <Toggle on={autostart} onClick={toggleAutostart} />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-fg-3">{m.settings.language}</p>
              <p className="text-xs text-fg-5">{m.settings.languageHint}</p>
            </div>
            <select
              aria-label={m.settings.language}
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

        {guard ? (
          <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
            <h3 className="font-medium text-fg-2">{m.settings.guard.title}</h3>
            <p className="text-xs leading-relaxed text-fg-4">
              {m.settings.guard.intro}
            </p>

            {[
              {
                key: "unset_global_identity" as const,
                label: m.settings.guard.unsetGlobal,
                hint: m.settings.guard.unsetGlobalHint,
              },
              {
                key: "manage_gitconfig_includes" as const,
                label: m.settings.guard.manageIncludes,
                hint: m.settings.guard.manageIncludesHint,
              },
              {
                key: "own_bare_ssh_hosts" as const,
                label: m.settings.guard.ownBareHosts,
                hint: m.settings.guard.ownBareHostsHint,
              },
            ].map((row) => (
              <div
                key={row.key}
                className="flex items-center justify-between gap-4"
              >
                <div>
                  <p className="text-sm text-fg-3">{row.label}</p>
                  <p className="text-xs text-fg-5">
                    {rich(row.hint, { codeClass: "text-fg-4" })}
                  </p>
                </div>
                <Toggle
                  on={guard[row.key]}
                  onClick={() => toggleGuard(row.key)}
                />
              </div>
            ))}

            {guardStatus && (
              <dl className="space-y-1 border-t border-bd pt-3 text-xs">
                <div className="flex justify-between gap-4">
                  <dt className="text-fg-4">{m.repos.globalIdentity}</dt>
                  <dd className="text-right text-fg-3">
                    {guardStatus.global_email || m.repos.globalNone}
                  </dd>
                </div>
                <div className="flex justify-between gap-4">
                  <dt className="text-fg-4">{m.repos.useConfigOnly}</dt>
                  <dd className="text-fg-3">
                    {guardStatus.use_config_only ? m.repos.on : m.repos.off}
                  </dd>
                </div>
                <div className="flex justify-between gap-4">
                  <dt className="text-fg-4">{m.repos.includesRegion}</dt>
                  <dd className="text-fg-3">
                    {guardStatus.includes_managed ? m.repos.on : m.repos.off}
                  </dd>
                </div>
              </dl>
            )}
          </div>
        ) : null}

        {openSshProbe?.available ? (
          <div className="space-y-3 rounded-lg border border-bd bg-raised-40 p-4">
            <h3 className="font-medium text-fg-2">
              {m.settings.tortoise.title}
            </h3>
            <p className="text-xs text-fg-4 leading-relaxed">
              {rich(m.settings.tortoise.intro)}
            </p>
            {openSshProbe.ssh_exe ? (
              <p className="text-xs text-fg-5">
                {rich(
                  fmt(m.settings.tortoise.detected, {
                    path: openSshProbe.ssh_exe,
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
                <p className="text-sm text-fg-3">
                  {m.settings.tortoise.toggle}
                </p>
                <p className="text-xs text-fg-5">
                  {rich(m.settings.tortoise.toggleHint, {
                    codeClass: "text-fg-4",
                  })}
                </p>
              </div>
              <Toggle
                on={useOpenSsh}
                onClick={() => setUseOpenSsh(!useOpenSsh)}
              />
            </div>
            {useOpenSsh && !openSshProbe.ssh_exe ? (
              <p className="text-xs text-red-600 dark:text-red-400">
                {rich(m.settings.tortoise.willFail)}
              </p>
            ) : null}
          </div>
        ) : null}

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
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="btn-primary"
        >
          {saving ? m.settings.saving : m.settings.save}
        </button>
        {saved && (
          <span className="text-sm text-success-fg">{m.settings.saved}</span>
        )}
        {saveError ? (
          <span className="max-w-md text-sm text-red-600 dark:text-red-400">
            {saveError}
          </span>
        ) : null}
      </div>
    </div>
  );
}
