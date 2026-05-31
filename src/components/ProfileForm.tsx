import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Profile,
  PlatformAccount,
  PlatformUser,
  SshKeyInfo,
  SshKeyPair,
  OAuthSettings,
  DeviceCodeResponse,
} from "../types";
import ConfirmDialog, { DialogAction } from "./ConfirmDialog";
import { copySshPublicKey } from "../copySshPublicKey";
import { useI18n, fmt, rich } from "../i18n";

const CopyIcon = () => (
  <svg
    className="h-3.5 w-3.5"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
    aria-hidden
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
    />
  </svg>
);

interface Props {
  profile: Profile | null;
  onSave: (profile: Profile) => void;
  onCancel: () => void;
  onSettings: () => void;
  onDelete: (id: string, deleteKeys: boolean) => void;
}

interface PlatformState {
  connected: boolean;
  connecting: boolean;
  token: string;
  username: string;
  gitName: string;
  gitEmail: string;
  publicEmail: string;
  noreplyEmail: string;
  sshPrivateKeyPath: string;
  sshPublicKeyPath: string;
  sshSource: "existing" | "generate";
  selectedKey: string;
  error: string;
  keyUploaded: boolean;
  deviceCode: DeviceCodeResponse | null;
}

function emptyPlatform(): PlatformState {
  return {
    connected: false,
    connecting: false,
    token: "",
    username: "",
    gitName: "",
    gitEmail: "",
    publicEmail: "",
    noreplyEmail: "",
    sshPrivateKeyPath: "",
    sshPublicKeyPath: "",
    sshSource: "generate",
    selectedKey: "",
    error: "",
    keyUploaded: false,
    deviceCode: null,
  };
}

function platformFromAccount(acc?: PlatformAccount): PlatformState {
  if (!acc) return emptyPlatform();
  return {
    connected: true,
    connecting: false,
    token: acc.token || "",
    username: acc.username,
    gitName: acc.git_name,
    gitEmail: acc.git_email,
    publicEmail: "",
    noreplyEmail: "",
    sshPrivateKeyPath: acc.ssh_private_key_path,
    sshPublicKeyPath: acc.ssh_public_key_path,
    sshSource: "existing",
    selectedKey: acc.ssh_private_key_path,
    error: "",
    keyUploaded: true,
    deviceCode: null,
  };
}

export default function ProfileForm({
  profile,
  onSave,
  onCancel,
  onSettings,
  onDelete,
}: Props) {
  const isEdit = profile !== null;
  const { m } = useI18n();

  const [name, setName] = useState(profile?.name || "");
  const [defaultPlatform, setDefaultPlatform] = useState(
    profile?.default_platform || "github",
  );
  const [gh, setGh] = useState<PlatformState>(
    platformFromAccount(profile?.github),
  );
  const [gl, setGl] = useState<PlatformState>(
    platformFromAccount(profile?.gitlab),
  );
  const [sshKeys, setSshKeys] = useState<SshKeyInfo[]>([]);
  const [saving, setSaving] = useState(false);
  const [disconnectTarget, setDisconnectTarget] = useState<{
    platform: "github" | "gitlab";
    keyPath: string;
    pubKeyPath: string;
    token: string;
  } | null>(null);
  const [error, setError] = useState("");
  const [settings, setSettings] = useState<OAuthSettings | null>(null);
  const [copiedPublicPath, setCopiedPublicPath] = useState<string | null>(null);
  const copyHintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const ghPollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const ghTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [ghCountdown, setGhCountdown] = useState(0);
  const ghCountdownRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function cancelGitHubAuth() {
    if (ghPollRef.current) {
      clearInterval(ghPollRef.current);
      ghPollRef.current = null;
    }
    if (ghTimeoutRef.current) {
      clearTimeout(ghTimeoutRef.current);
      ghTimeoutRef.current = null;
    }
    if (ghCountdownRef.current) {
      clearInterval(ghCountdownRef.current);
      ghCountdownRef.current = null;
    }
    setGhCountdown(0);
    updateGh({ connecting: false, deviceCode: null, error: "" });
  }

  const [glCountdown, setGlCountdown] = useState(0);
  const glCountdownRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const glCancelledRef = useRef(false);
  const glConnectingRef = useRef(false);

  useEffect(() => {
    glConnectingRef.current = gl.connecting;
  }, [gl.connecting]);

  useEffect(() => {
    return () => {
      if (copyHintTimerRef.current) clearTimeout(copyHintTimerRef.current);
    };
  }, []);

  async function handleCopyPublicKey(publicKeyPath: string) {
    try {
      await copySshPublicKey(publicKeyPath);
      if (copyHintTimerRef.current) clearTimeout(copyHintTimerRef.current);
      setCopiedPublicPath(publicKeyPath);
      copyHintTimerRef.current = setTimeout(() => {
        setCopiedPublicPath(null);
        copyHintTimerRef.current = null;
      }, 2000);
    } catch {
      /* ignore */
    }
  }

  function abortGitLabOAuthBackend() {
    void invoke("gitlab_oauth_abort").catch(() => {});
  }

  function cancelGitLabAuth() {
    glCancelledRef.current = true;
    abortGitLabOAuthBackend();
    if (glCountdownRef.current) {
      clearInterval(glCountdownRef.current);
      glCountdownRef.current = null;
    }
    setGlCountdown(0);
    updateGl({ connecting: false, error: "" });
  }

  function handleProfileCancel() {
    if (gl.connecting) {
      glCancelledRef.current = true;
      abortGitLabOAuthBackend();
      if (glCountdownRef.current) {
        clearInterval(glCountdownRef.current);
        glCountdownRef.current = null;
      }
      setGlCountdown(0);
      updateGl({ connecting: false, error: "" });
    }
    if (gh.connecting || gh.deviceCode) {
      cancelGitHubAuth();
    }
    onCancel();
  }

  const handleProfileCancelRef = useRef(handleProfileCancel);
  handleProfileCancelRef.current = handleProfileCancel;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !disconnectTarget) {
        handleProfileCancelRef.current();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [disconnectTarget]);

  useEffect(() => {
    invoke<SshKeyInfo[]>("list_ssh_keys")
      .then(setSshKeys)
      .catch(() => {});
    invoke<OAuthSettings>("get_settings")
      .then(setSettings)
      .catch(() => {});
    return () => {
      if (ghPollRef.current) clearInterval(ghPollRef.current);
      if (ghTimeoutRef.current) clearTimeout(ghTimeoutRef.current);
      if (ghCountdownRef.current) clearInterval(ghCountdownRef.current);
      if (glCountdownRef.current) clearInterval(glCountdownRef.current);
      if (glConnectingRef.current) {
        void invoke("gitlab_oauth_abort").catch(() => {});
      }
    };
  }, []);

  const updateGh = (p: Partial<PlatformState>) =>
    setGh((prev) => ({ ...prev, ...p }));
  const updateGl = (p: Partial<PlatformState>) =>
    setGl((prev) => ({ ...prev, ...p }));

  async function connectGitHub() {
    if (!settings?.github_client_id) {
      updateGh({ error: "settings_required" });
      return;
    }
    updateGh({ connecting: true, error: "" });
    try {
      const device = await invoke<DeviceCodeResponse>("github_oauth_start", {
        clientId: settings.github_client_id,
      });
      updateGh({ deviceCode: device });
      await openUrl(device.verification_uri);

      const expiresIn = device.expires_in || 900;
      setGhCountdown(expiresIn);
      ghCountdownRef.current = setInterval(() => {
        setGhCountdown((prev) => {
          if (prev <= 1) return 0;
          return prev - 1;
        });
      }, 1000);

      ghTimeoutRef.current = setTimeout(() => {
        if (ghPollRef.current) {
          clearInterval(ghPollRef.current);
          ghPollRef.current = null;
        }
        if (ghCountdownRef.current) {
          clearInterval(ghCountdownRef.current);
          ghCountdownRef.current = null;
        }
        setGhCountdown(0);
        updateGh({
          connecting: false,
          deviceCode: null,
          error: m.form.authTimedOut,
        });
      }, expiresIn * 1000);

      ghPollRef.current = setInterval(
        async () => {
          try {
            const token = await invoke<string | null>("github_oauth_poll", {
              clientId: settings.github_client_id,
              deviceCode: device.device_code,
            });
            if (token) {
              if (ghPollRef.current) {
                clearInterval(ghPollRef.current);
                ghPollRef.current = null;
              }
              if (ghTimeoutRef.current) {
                clearTimeout(ghTimeoutRef.current);
                ghTimeoutRef.current = null;
              }
              if (ghCountdownRef.current) {
                clearInterval(ghCountdownRef.current);
                ghCountdownRef.current = null;
              }
              setGhCountdown(0);
              const user = await invoke<PlatformUser>("verify_platform_token", {
                platform: "github",
                token,
              });
              const noreply = user.noreply_email || "";
              const pubEmail = user.email || "";
              updateGh({
                connecting: false,
                connected: true,
                deviceCode: null,
                token,
                username: user.username,
                gitName: user.name || user.username,
                gitEmail: noreply || pubEmail,
                publicEmail: pubEmail,
                noreplyEmail: noreply,
              });
            }
          } catch (e) {
            if (ghPollRef.current) {
              clearInterval(ghPollRef.current);
              ghPollRef.current = null;
            }
            if (ghTimeoutRef.current) {
              clearTimeout(ghTimeoutRef.current);
              ghTimeoutRef.current = null;
            }
            if (ghCountdownRef.current) {
              clearInterval(ghCountdownRef.current);
              ghCountdownRef.current = null;
            }
            setGhCountdown(0);
            updateGh({ connecting: false, deviceCode: null, error: String(e) });
          }
        },
        (device.interval + 1) * 1000,
      );
    } catch (e) {
      updateGh({ connecting: false, error: String(e) });
    }
  }

  async function connectGitLab() {
    if (!settings?.gitlab_client_id) {
      updateGl({ error: "settings_required" });
      return;
    }
    glCancelledRef.current = false;
    updateGl({ connecting: true, error: "" });

    setGlCountdown(120);
    glCountdownRef.current = setInterval(() => {
      setGlCountdown((prev) => (prev <= 1 ? 0 : prev - 1));
    }, 1000);

    try {
      const token = await invoke<string>("gitlab_oauth_connect", {
        clientId: settings.gitlab_client_id,
      });
      if (glCountdownRef.current) {
        clearInterval(glCountdownRef.current);
        glCountdownRef.current = null;
      }
      setGlCountdown(0);
      if (glCancelledRef.current) return;
      const user = await invoke<PlatformUser>("verify_platform_token", {
        platform: "gitlab",
        token,
      });
      if (glCancelledRef.current) return;
      const noreply = user.noreply_email || "";
      const pubEmail = user.email || "";
      updateGl({
        connecting: false,
        connected: true,
        token,
        username: user.username,
        gitName: user.name || user.username,
        gitEmail: noreply || pubEmail,
        publicEmail: pubEmail,
        noreplyEmail: noreply,
      });
    } catch (e) {
      if (glCountdownRef.current) {
        clearInterval(glCountdownRef.current);
        glCountdownRef.current = null;
      }
      setGlCountdown(0);
      if (glCancelledRef.current) return;
      updateGl({ connecting: false, error: String(e) });
    }
  }

  async function generateAndUpload(
    platform: "github" | "gitlab",
    section: PlatformState,
    update: (p: Partial<PlatformState>) => void,
  ) {
    if (!section.token) {
      update({ error: m.form.errConnectFirst });
      return;
    }
    update({ error: "" });
    try {
      const pair = await invoke<SshKeyPair>("generate_and_upload_key", {
        platform,
        token: section.token,
        username: section.username,
        email: section.gitEmail || "git@account-switcher",
      });
      update({
        sshPrivateKeyPath: pair.private_key_path,
        sshPublicKeyPath: pair.public_key_path,
        keyUploaded: true,
      });
      const keys = await invoke<SshKeyInfo[]>("list_ssh_keys");
      setSshKeys(keys);
    } catch (e) {
      update({ error: String(e) });
    }
  }

  function selectKey(
    key: SshKeyInfo,
    update: (p: Partial<PlatformState>) => void,
  ) {
    update({
      selectedKey: key.private_key_path,
      sshPrivateKeyPath: key.private_key_path,
      sshPublicKeyPath: key.public_key_path,
    });
  }

  async function uploadExistingKey(
    platform: "github" | "gitlab",
    section: PlatformState,
    update: (p: Partial<PlatformState>) => void,
  ) {
    if (!section.token || !section.sshPublicKeyPath) return;
    update({ error: "" });
    try {
      const keyContent = await invoke<string>("read_public_key", {
        path: section.sshPublicKeyPath,
      });
      await invoke("upload_ssh_key_to_platform", {
        platform,
        token: section.token,
        title: `git-account-manager: ${name}`,
        keyContent,
      });
      update({ keyUploaded: true });
    } catch (e) {
      update({ error: String(e) });
    }
  }

  async function handleSave() {
    if (!name.trim()) {
      setError(m.form.errProfileName);
      return;
    }
    if (!gh.connected && !gl.connected) {
      setError(m.form.errConnectOne);
      return;
    }
    setSaving(true);
    setError("");

    const buildAccount = (s: PlatformState): PlatformAccount | undefined => {
      if (!s.connected || !s.sshPrivateKeyPath) return undefined;
      return {
        username: s.username,
        git_name: s.gitName,
        git_email: s.gitEmail,
        ssh_private_key_path: s.sshPrivateKeyPath,
        ssh_public_key_path: s.sshPublicKeyPath,
        token: s.token || undefined,
      };
    };

    const p: Profile = {
      id: profile?.id || crypto.randomUUID(),
      name: name.trim(),
      default_platform: defaultPlatform,
      github: buildAccount(gh),
      gitlab: buildAccount(gl),
      is_active: profile?.is_active || false,
    };

    try {
      await invoke("save_profile", { profile: p });
      onSave(p);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  function renderError(err: string, platform: string) {
    if (err === "settings_required") {
      return (
        <p className="text-xs text-danger-fg">
          {rich(
            fmt(m.form.errSettingsRequired, {
              platform: platform === "github" ? "GitHub" : "GitLab",
            }),
            { onLink: onSettings },
          )}
        </p>
      );
    }
    return <p className="text-xs text-danger-fg">{err}</p>;
  }

  function renderPlatform(
    label: string,
    platform: "github" | "gitlab",
    section: PlatformState,
    update: (p: Partial<PlatformState>) => void,
    onConnect: () => void,
  ) {
    return (
      <div className="rounded-lg border border-bd bg-raised-40 p-4">
        <div className="mb-3 flex items-center justify-between">
          <h4 className="font-medium text-fg-2">{label}</h4>
          {section.connected && (
            <button
              onClick={() =>
                openUrl(
                  platform === "github"
                    ? `https://github.com/${section.username}`
                    : `https://gitlab.com/${section.username}`,
                )
              }
              className="text-sm text-link hover:text-link-hover hover:underline"
            >
              @{section.username}
            </button>
          )}
        </div>

        {!section.connected ? (
          <div className="space-y-3">
            {section.deviceCode ? (
              <div className="space-y-2 rounded-md border border-info-border bg-info-bg p-3">
                <p className="text-sm text-fg-3">{m.form.enterCode}</p>
                <p className="font-mono text-2xl font-bold tracking-widest text-link">
                  {section.deviceCode.user_code}
                </p>
                <div className="flex items-center justify-between">
                  <p className="text-xs text-fg-4">
                    {m.form.waitingAuth}
                    {ghCountdown > 0 && (
                      <span className="ml-1 text-fg-5">
                        ({Math.floor(ghCountdown / 60)}:
                        {String(ghCountdown % 60).padStart(2, "0")})
                      </span>
                    )}
                  </p>
                  <button
                    onClick={cancelGitHubAuth}
                    className="rounded-md bg-subtle px-3 py-1 text-xs text-fg-3 transition-colors hover:bg-hover hover:text-fg"
                  >
                    {m.form.cancel}
                  </button>
                </div>
              </div>
            ) : section.connecting ? (
              <div className="space-y-2 rounded-md border border-info-border bg-info-bg p-3">
                <div className="flex items-center justify-between">
                  <p className="text-xs text-fg-4">
                    {m.form.waitingBrowser}
                    {platform === "gitlab" && glCountdown > 0 && (
                      <span className="ml-1 text-fg-5">
                        ({Math.floor(glCountdown / 60)}:
                        {String(glCountdown % 60).padStart(2, "0")})
                      </span>
                    )}
                  </p>
                  {platform === "gitlab" && (
                    <button
                      onClick={cancelGitLabAuth}
                      className="rounded-md bg-subtle px-3 py-1 text-xs text-fg-3 transition-colors hover:bg-hover hover:text-fg"
                    >
                      {m.form.cancel}
                    </button>
                  )}
                </div>
                {platform === "gitlab" && (
                  <p className="text-xs text-fg-5">
                    {m.form.gitlabClipboardHint}
                  </p>
                )}
              </div>
            ) : (
              <button
                onClick={onConnect}
                className="w-full rounded-md bg-blue-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-blue-500"
              >
                {fmt(m.form.connectWith, { platform: label })}
              </button>
            )}
            {section.error && renderError(section.error, platform)}
          </div>
        ) : (
          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-xs text-fg-4">
                {m.form.gitName}
              </label>
              <input
                type="text"
                value={section.gitName}
                onChange={(e) => update({ gitName: e.target.value })}
                className="w-full rounded-md border border-bd-s bg-input px-2.5 py-1.5 text-sm text-fg outline-none focus:border-blue-500"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-fg-4">
                {m.form.gitEmail}
              </label>
              {section.noreplyEmail || section.publicEmail ? (
                <div className="space-y-1.5">
                  {section.noreplyEmail && (
                    <button
                      onClick={() => update({ gitEmail: section.noreplyEmail })}
                      className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-xs transition-colors ${
                        section.gitEmail === section.noreplyEmail
                          ? "border-selected-border bg-selected-bg text-selected-fg"
                          : "border-bd-s bg-input text-fg-3 hover:border-bd-s"
                      }`}
                    >
                      <span className="shrink-0 rounded bg-badge-ok-bg px-1 py-0.5 text-[10px] font-medium text-badge-ok-fg">
                        {m.form.noreplyBadge}
                      </span>
                      <span className="truncate">{section.noreplyEmail}</span>
                    </button>
                  )}
                  {section.publicEmail &&
                    section.publicEmail !== section.noreplyEmail && (
                      <button
                        onClick={() =>
                          update({ gitEmail: section.publicEmail })
                        }
                        className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-xs transition-colors ${
                          section.gitEmail === section.publicEmail
                            ? "border-selected-border bg-selected-bg text-selected-fg"
                            : "border-bd-s bg-input text-fg-3 hover:border-bd-s"
                        }`}
                      >
                        <span className="shrink-0 rounded bg-subtle px-1 py-0.5 text-[10px] font-medium text-fg-3">
                          {m.form.publicBadge}
                        </span>
                        <span className="truncate">{section.publicEmail}</span>
                      </button>
                    )}
                  <input
                    type="text"
                    value={section.gitEmail}
                    onChange={(e) => update({ gitEmail: e.target.value })}
                    placeholder={m.form.customEmailPlaceholder}
                    className="w-full rounded-md border border-bd-s bg-input px-2.5 py-1.5 text-xs text-fg outline-none focus:border-blue-500"
                  />
                </div>
              ) : (
                <input
                  type="text"
                  value={section.gitEmail}
                  onChange={(e) => update({ gitEmail: e.target.value })}
                  className="w-full rounded-md border border-bd-s bg-input px-2.5 py-1.5 text-sm text-fg outline-none focus:border-blue-500"
                />
              )}
            </div>

            <div>
              <label className="mb-1 block text-xs text-fg-4">
                {m.form.sshKey}
              </label>
              {section.sshPrivateKeyPath && section.keyUploaded ? (
                <div className="flex flex-col gap-2 rounded-md border border-active-border bg-active-bg px-3 py-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex min-w-0 items-center gap-2">
                    <svg
                      className="h-4 w-4 shrink-0 text-success-icon"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M5 13l4 4L19 7"
                      />
                    </svg>
                    <span className="text-xs text-success-fg">
                      {fmt(m.form.uploadedTo, {
                        file:
                          section.sshPrivateKeyPath.split(/[\\/]/).pop() || "",
                        platform: label,
                      })}
                    </span>
                  </div>
                  {section.sshPublicKeyPath ? (
                    <button
                      type="button"
                      onClick={() =>
                        handleCopyPublicKey(section.sshPublicKeyPath)
                      }
                      className="inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md border border-bd-s bg-input px-2.5 py-1 text-xs font-medium text-fg-2 transition-colors hover:bg-hover"
                    >
                      <CopyIcon />
                      {copiedPublicPath === section.sshPublicKeyPath
                        ? m.form.copied
                        : m.form.copyPublicKey}
                    </button>
                  ) : null}
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <button
                      onClick={() => update({ sshSource: "generate" })}
                      className={`rounded-md px-2 py-1 text-xs ${section.sshSource === "generate" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
                    >
                      {m.form.generateUpload}
                    </button>
                    <button
                      onClick={() => update({ sshSource: "existing" })}
                      className={`rounded-md px-2 py-1 text-xs ${section.sshSource === "existing" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
                    >
                      {m.form.useExisting}
                    </button>
                  </div>

                  {section.sshSource === "generate" ? (
                    <button
                      onClick={() =>
                        generateAndUpload(platform, section, update)
                      }
                      className="w-full rounded-md bg-emerald-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-emerald-500"
                    >
                      {fmt(m.form.generateAddTo, { platform: label })}
                    </button>
                  ) : (
                    <div className="space-y-2">
                      {sshKeys.length === 0 ? (
                        <p className="text-xs text-fg-5">{m.form.noSshKeys}</p>
                      ) : (
                        <div className="max-h-28 space-y-1 overflow-y-auto">
                          {sshKeys.map((k) => (
                            <div
                              key={k.private_key_path}
                              className="flex gap-1"
                            >
                              <button
                                type="button"
                                onClick={() => selectKey(k, update)}
                                className={`min-w-0 flex-1 rounded-md border px-2 py-1.5 text-left text-xs transition-colors ${
                                  section.selectedKey === k.private_key_path
                                    ? "border-selected-border bg-selected-bg text-selected-fg"
                                    : "border-bd-s bg-input text-fg-3 hover:border-bd-s"
                                }`}
                              >
                                {k.name}
                              </button>
                              <button
                                type="button"
                                title={m.form.copyPublicKey}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  void handleCopyPublicKey(k.public_key_path);
                                }}
                                className={`flex shrink-0 items-center justify-center rounded-md border px-2 py-1.5 transition-colors ${
                                  copiedPublicPath === k.public_key_path
                                    ? "border-selected-border bg-selected-bg text-selected-fg"
                                    : "border-bd-s bg-input text-fg-4 hover:border-bd-s hover:text-fg-2"
                                }`}
                              >
                                <CopyIcon />
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                      {section.sshPrivateKeyPath && !section.keyUploaded && (
                        <button
                          onClick={() =>
                            uploadExistingKey(platform, section, update)
                          }
                          className="rounded-md bg-subtle px-3 py-1.5 text-xs text-fg-2 hover:bg-hover"
                        >
                          {fmt(m.form.uploadTo, { platform: label })}
                        </button>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>

            {section.error && renderError(section.error, platform)}

            <button
              onClick={() =>
                setDisconnectTarget({
                  platform,
                  keyPath: section.sshPrivateKeyPath,
                  pubKeyPath: section.sshPublicKeyPath,
                  token: section.token,
                })
              }
              className="text-xs text-danger-fg hover:underline"
            >
              {m.form.disconnect}
            </button>
          </div>
        )}
      </div>
    );
  }

  async function handleDisconnect(deleteKeys: boolean) {
    if (!disconnectTarget) return;
    const { platform, keyPath, pubKeyPath, token } = disconnectTarget;
    const update = platform === "github" ? updateGh : updateGl;

    if (deleteKeys && keyPath) {
      if (token && pubKeyPath) {
        await invoke("remove_ssh_key_from_platform", {
          platform,
          token,
          publicKeyPath: pubKeyPath,
        }).catch(() => {});
      }
      await invoke("delete_ssh_keys", { paths: [keyPath] }).catch(() => {});
    }
    update(emptyPlatform());
    setDisconnectTarget(null);

    if (profile) {
      const otherGh = platform === "github" ? false : gh.connected;
      const otherGl = platform === "gitlab" ? false : gl.connected;

      if (!otherGh && !otherGl) {
        onDelete(profile.id, false);
        return;
      }

      const updatedGh = platform === "github" ? undefined : profile.github;
      const updatedGl = platform === "gitlab" ? undefined : profile.gitlab;
      const updated = { ...profile, github: updatedGh, gitlab: updatedGl };
      try {
        await invoke("save_profile", { profile: updated });
        onSave(updated);
      } catch {
        /* keep form open on error */
      }
    }
  }

  const disconnectKeyName = disconnectTarget?.keyPath
    ? disconnectTarget.keyPath.split(/[\\/]/).pop() || ""
    : "";

  const disconnectActions: DialogAction[] = [
    ...(disconnectTarget?.keyPath
      ? [
          {
            label: m.form.disconnectAndDelete,
            variant: "danger" as const,
            onClick: () => handleDisconnect(true),
          },
          {
            label: m.form.disconnectKeep,
            variant: "default" as const,
            onClick: () => handleDisconnect(false),
          },
        ]
      : [
          {
            label: m.form.disconnect,
            variant: "danger" as const,
            onClick: () => handleDisconnect(false),
          },
        ]),
    {
      label: m.form.cancel,
      variant: "cancel" as const,
      onClick: () => setDisconnectTarget(null),
    },
  ];

  return (
    <>
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between border-b border-bd px-6 py-4">
          <h2 className="text-lg font-semibold text-fg">
            {isEdit ? m.form.editTitle : m.form.newTitle}
          </h2>
          <button
            onClick={handleProfileCancel}
            className="text-fg-4 hover:text-fg-2"
          >
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
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto p-6">
          <div>
            <label className="mb-1 block text-sm font-medium text-fg-3">
              {m.form.profileName}
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={m.form.profileNamePlaceholder}
              className="w-full rounded-md border border-bd-s bg-input px-3 py-2 text-sm text-fg outline-none focus:border-blue-500"
            />
          </div>

          {renderPlatform("GitHub", "github", gh, updateGh, connectGitHub)}
          {renderPlatform("GitLab", "gitlab", gl, updateGl, connectGitLab)}

          {gh.connected && gl.connected && (
            <div className="rounded-lg border border-bd bg-raised-40 p-4">
              <label className="mb-1 block text-sm font-medium text-fg-3">
                {m.form.defaultIdentity}
              </label>
              <p className="mb-1 text-xs text-fg-5">
                {rich(m.form.defaultIdentityHint1, { codeClass: "text-fg-4" })}
              </p>
              <p className="mb-3 text-xs text-fg-5">
                {m.form.defaultIdentityHint2}
              </p>
              <div className="mb-3 flex gap-3">
                {(["github", "gitlab"] as const).map((p) => (
                  <button
                    key={p}
                    onClick={() => setDefaultPlatform(p)}
                    className={`rounded-md px-3 py-1.5 text-sm ${
                      defaultPlatform === p
                        ? "bg-blue-600 text-white"
                        : "bg-subtle text-fg-3"
                    }`}
                  >
                    {p === "github" ? "GitHub" : "GitLab"}
                  </button>
                ))}
              </div>
              <p className="text-xs text-fg-4">
                {m.form.activeLabel}{" "}
                <span className="font-medium text-fg-2">
                  {defaultPlatform === "github" ? gh.gitName : gl.gitName}
                </span>{" "}
                <span className="text-fg-5">
                  &lt;
                  {defaultPlatform === "github" ? gh.gitEmail : gl.gitEmail}
                  &gt;
                </span>
              </p>
            </div>
          )}

          {error && (
            <div className="rounded-md bg-danger-bg p-3 text-sm text-danger-fg">
              {error}
            </div>
          )}
        </div>

        <div className="flex gap-3 border-t border-bd px-6 py-4">
          <button
            onClick={handleSave}
            disabled={saving}
            className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
          >
            {saving
              ? m.form.saving
              : isEdit
                ? m.form.saveChanges
                : m.form.createProfile}
          </button>
          <button
            onClick={handleProfileCancel}
            className="rounded-md bg-subtle px-4 py-2 text-sm font-medium text-fg-2 transition-colors hover:bg-hover"
          >
            {m.form.cancel}
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={disconnectTarget !== null}
        title={fmt(m.form.disconnectTitle, {
          platform:
            disconnectTarget?.platform === "github" ? "GitHub" : "GitLab",
        })}
        actions={disconnectActions}
      >
        <p className="mb-3 text-sm text-fg-3">{m.form.disconnectBody}</p>
        {disconnectKeyName && (
          <div className="space-y-1">
            <p className="text-xs text-fg-4">{m.form.disconnectKeyLabel}</p>
            <div className="rounded bg-raised px-2 py-1 font-mono text-xs text-fg-3">
              {disconnectKeyName}
            </div>
          </div>
        )}
      </ConfirmDialog>
    </>
  );
}
