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
          error: "Authorization timed out. Please try again.",
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
      update({ error: "Connect to platform first" });
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
      setError("Profile name is required");
      return;
    }
    if (!gh.connected && !gl.connected) {
      setError("Connect at least one platform");
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
          Configure {platform === "github" ? "GitHub" : "GitLab"} OAuth Client
          ID in{" "}
          <button
            onClick={onSettings}
            className="font-medium text-link underline hover:text-link-hover"
          >
            Settings
          </button>{" "}
          first
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
                <p className="text-sm text-fg-3">Enter this code on GitHub:</p>
                <p className="font-mono text-2xl font-bold tracking-widest text-link">
                  {section.deviceCode.user_code}
                </p>
                <div className="flex items-center justify-between">
                  <p className="text-xs text-fg-4">
                    Waiting for authorization...
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
                    Cancel
                  </button>
                </div>
              </div>
            ) : section.connecting ? (
              <div className="space-y-2 rounded-md border border-info-border bg-info-bg p-3">
                <div className="flex items-center justify-between">
                  <p className="text-xs text-fg-4">
                    Waiting for browser authorization...
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
                      Cancel
                    </button>
                  )}
                </div>
                {platform === "gitlab" && (
                  <p className="text-xs text-fg-5">
                    The sign-in link was copied to your clipboard — paste it in
                    a browser if the page did not open.
                  </p>
                )}
              </div>
            ) : (
              <button
                onClick={onConnect}
                className="w-full rounded-md bg-blue-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-blue-500"
              >
                Connect with {label}
              </button>
            )}
            {section.error && renderError(section.error, platform)}
          </div>
        ) : (
          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-xs text-fg-4">Git Name</label>
              <input
                type="text"
                value={section.gitName}
                onChange={(e) => update({ gitName: e.target.value })}
                className="w-full rounded-md border border-bd-s bg-input px-2.5 py-1.5 text-sm text-fg outline-none focus:border-blue-500"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-fg-4">Git Email</label>
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
                        noreply
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
                          public
                        </span>
                        <span className="truncate">{section.publicEmail}</span>
                      </button>
                    )}
                  <input
                    type="text"
                    value={section.gitEmail}
                    onChange={(e) => update({ gitEmail: e.target.value })}
                    placeholder="or enter custom email"
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
              <label className="mb-1 block text-xs text-fg-4">SSH Key</label>
              {section.sshPrivateKeyPath && section.keyUploaded ? (
                <div className="flex items-center gap-2 rounded-md border border-active-border bg-active-bg px-3 py-2">
                  <svg
                    className="h-4 w-4 text-success-icon"
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
                    {section.sshPrivateKeyPath.split(/[\\/]/).pop()} — uploaded
                    to {label}
                  </span>
                </div>
              ) : (
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <button
                      onClick={() => update({ sshSource: "generate" })}
                      className={`rounded-md px-2 py-1 text-xs ${section.sshSource === "generate" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
                    >
                      Generate & Upload
                    </button>
                    <button
                      onClick={() => update({ sshSource: "existing" })}
                      className={`rounded-md px-2 py-1 text-xs ${section.sshSource === "existing" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
                    >
                      Use Existing
                    </button>
                  </div>

                  {section.sshSource === "generate" ? (
                    <button
                      onClick={() =>
                        generateAndUpload(platform, section, update)
                      }
                      className="w-full rounded-md bg-emerald-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-emerald-500"
                    >
                      Generate SSH Key & Add to {label}
                    </button>
                  ) : (
                    <div className="space-y-2">
                      {sshKeys.length === 0 ? (
                        <p className="text-xs text-fg-5">
                          No SSH keys found in ~/.ssh
                        </p>
                      ) : (
                        <div className="max-h-28 space-y-1 overflow-y-auto">
                          {sshKeys.map((k) => (
                            <button
                              key={k.private_key_path}
                              onClick={() => selectKey(k, update)}
                              className={`w-full rounded-md border px-2 py-1.5 text-left text-xs transition-colors ${
                                section.selectedKey === k.private_key_path
                                  ? "border-selected-border bg-selected-bg text-selected-fg"
                                  : "border-bd-s bg-input text-fg-3 hover:border-bd-s"
                              }`}
                            >
                              {k.name}
                            </button>
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
                          Upload to {label}
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
              Disconnect
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
            label: "Disconnect and delete SSH key",
            variant: "danger" as const,
            onClick: () => handleDisconnect(true),
          },
          {
            label: "Disconnect, keep SSH key",
            variant: "default" as const,
            onClick: () => handleDisconnect(false),
          },
        ]
      : [
          {
            label: "Disconnect",
            variant: "danger" as const,
            onClick: () => handleDisconnect(false),
          },
        ]),
    {
      label: "Cancel",
      variant: "cancel" as const,
      onClick: () => setDisconnectTarget(null),
    },
  ];

  return (
    <>
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between border-b border-bd px-6 py-4">
          <h2 className="text-lg font-semibold text-fg">
            {isEdit ? "Edit Profile" : "New Profile"}
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
              Profile Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Personal, Work"
              className="w-full rounded-md border border-bd-s bg-input px-3 py-2 text-sm text-fg outline-none focus:border-blue-500"
            />
          </div>

          {renderPlatform("GitHub", "github", gh, updateGh, connectGitHub)}
          {renderPlatform("GitLab", "gitlab", gl, updateGl, connectGitLab)}

          {gh.connected && gl.connected && (
            <div className="rounded-lg border border-bd bg-raised-40 p-4">
              <label className="mb-1 block text-sm font-medium text-fg-3">
                Default Git Identity
              </label>
              <p className="mb-1 text-xs text-fg-5">
                Since both platforms are connected, choose which identity to use
                for{" "}
                <code className="text-fg-4">git config --global user.name</code>{" "}
                and <code className="text-fg-4">user.email</code>.
              </p>
              <p className="mb-3 text-xs text-fg-5">
                This email will be used in all your commits by default. Select a
                platform below to switch.
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
                Active:{" "}
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
            {saving ? "Saving..." : isEdit ? "Save Changes" : "Create Profile"}
          </button>
          <button
            onClick={handleProfileCancel}
            className="rounded-md bg-subtle px-4 py-2 text-sm font-medium text-fg-2 transition-colors hover:bg-hover"
          >
            Cancel
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={disconnectTarget !== null}
        title={`Disconnect ${disconnectTarget?.platform === "github" ? "GitHub" : "GitLab"}?`}
        actions={disconnectActions}
      >
        <p className="mb-3 text-sm text-fg-3">
          This action cannot be undone. The OAuth token will be removed.
        </p>
        {disconnectKeyName && (
          <div className="space-y-1">
            <p className="text-xs text-fg-4">Associated SSH key:</p>
            <div className="rounded bg-raised px-2 py-1 font-mono text-xs text-fg-3">
              {disconnectKeyName}
            </div>
          </div>
        )}
      </ConfirmDialog>
    </>
  );
}
