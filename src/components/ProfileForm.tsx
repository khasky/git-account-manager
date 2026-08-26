import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../api";
import { copySshPublicKey } from "../copySshPublicKey";
import { fmt, rich, useI18n } from "../i18n";
import { PLATFORM_LABEL, PLATFORMS } from "../platforms";
import { buildRepoPlan } from "../repoPlan";
import type {
  DiscoveredRepo,
  GitIdentity,
  OAuthSettings,
  PlatformAccount,
  PlatformId,
  Profile,
  RepoBinding,
  RepoRoot,
  RepoStatus,
  SshKeyInfo,
} from "../types";
import ConfirmDialog, { type DialogAction } from "./ConfirmDialog";
import { CloseIcon } from "./icons";
import PlatformSection, {
  emptyPlatform,
  type PlatformState,
} from "./PlatformSection";
import ProfileRepos from "./ProfileRepos";
import Spinner from "./Spinner";

interface Props {
  profile: Profile | null;
  prefill?: GitIdentity | null;
  onSave: (profile: Profile) => void;
  onCancel: () => void;
  onSettings: () => void;
  onDelete: (id: string, deleteKeys: boolean) => void;
}

const COPY_HINT_MS = 2000;

/** How long the GitLab browser flow waits before giving up, matching the
 *  backend's own callback timeout. */
const GITLAB_TIMEOUT_S = 120;

/** GitHub's device flow does not tell us how long it will take; this is its
 *  documented default, used only until the real value arrives. */
const GITHUB_FALLBACK_EXPIRY_S = 900;

function platformFromAccount(acc?: PlatformAccount): PlatformState {
  if (!acc) return emptyPlatform();
  return {
    ...emptyPlatform(),
    connected: true,
    username: acc.username,
    gitName: acc.git_name,
    gitEmail: acc.git_email,
    sshPrivateKeyPath: acc.ssh_private_key_path,
    sshPublicKeyPath: acc.ssh_public_key_path,
    sshSource: "existing",
    selectedKey: acc.ssh_private_key_path,
    keyUploaded: true,
  };
}

function noCountdowns(): Record<PlatformId, number> {
  return { github: 0, gitlab: 0, bitbucket: 0 };
}

export default function ProfileForm({
  profile,
  prefill,
  onSave,
  onCancel,
  onSettings,
  onDelete,
}: Props) {
  const isEdit = profile !== null;
  const { m } = useI18n();
  const [profileId] = useState(() => profile?.id || crypto.randomUUID());
  const initialPlatformsRef = useRef<Record<PlatformId, boolean>>({
    github: Boolean(profile?.github),
    gitlab: Boolean(profile?.gitlab),
    bitbucket: Boolean(profile?.bitbucket),
  });

  const [name, setName] = useState(profile?.name || prefill?.name || "");
  const [importedEmail, setImportedEmail] = useState(prefill?.email || "");
  const [defaultPlatform, setDefaultPlatform] = useState<PlatformId>(
    profile?.default_platform ?? "github",
  );

  // One record rather than three parallel `gh`/`gl`/`bb` states with three
  // setters: every handler used to exist in triplicate, and a change to one
  // that was not mirrored into the other two was invisible.
  const [sections, setSections] = useState<Record<PlatformId, PlatformState>>(
    () => ({
      github: platformFromAccount(profile?.github),
      gitlab: platformFromAccount(profile?.gitlab),
      bitbucket: platformFromAccount(profile?.bitbucket),
    }),
  );

  const update = useCallback(
    (platform: PlatformId, patch: Partial<PlatformState>) => {
      setSections((prev) => ({
        ...prev,
        [platform]: { ...prev[platform], ...patch },
      }));
    },
    [],
  );

  // Bitbucket connects via a pasted Atlassian API token (email + token),
  // not OAuth — these hold the two inputs until "Connect" combines them.
  const [bbEmail, setBbEmail] = useState("");
  const [bbToken, setBbToken] = useState("");
  const [sshKeys, setSshKeys] = useState<SshKeyInfo[]>([]);
  const [saving, setSaving] = useState(false);
  const [disconnectTarget, setDisconnectTarget] = useState<{
    platform: PlatformId;
    keyPath: string;
    pubKeyPath: string;
  } | null>(null);
  const [error, setError] = useState("");
  const [settings, setSettings] = useState<OAuthSettings | null>(null);
  const [copiedPublicPath, setCopiedPublicPath] = useState<string | null>(null);
  const copyHintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [countdown, setCountdown] =
    useState<Record<PlatformId, number>>(noCountdowns);

  // Folders and bindings are held as a draft and written only by Save, so
  // Cancel leaves no repository touched — and so a profile that does not exist
  // on disk yet can still have its folders configured before it is created.
  const [roots, setRoots] = useState<RepoRoot[]>([]);
  const [repos, setRepos] = useState<DiscoveredRepo[]>([]);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [storedBindings, setStoredBindings] = useState<RepoBinding[]>([]);
  const [statuses, setStatuses] = useState<RepoStatus[]>([]);
  // The doctor reads every bound repository from disk, so the section announces
  // itself as loading rather than rendering an empty state it is about to
  // contradict.
  const [reposLoading, setReposLoading] = useState(true);

  const loadRepoState = useCallback(async () => {
    setReposLoading(true);
    try {
      const state = await api.getRepoState();
      setRoots(state.roots.filter((r) => r.profile_id === profileId));
      setStoredBindings(
        state.bindings.filter((b) => b.profile_id === profileId),
      );
    } finally {
      setReposLoading(false);
    }

    // Reading the folders costs a file read; the doctor reads every bound
    // repository from disk and takes orders of magnitude longer. Awaiting both
    // together held the folders back for as long as the slower one, so the
    // report is fetched afterwards and fills its own list when it lands.
    const report = await api.doctor();
    setStatuses(report.repos.filter((r) => r.profile_id === profileId));
  }, [profileId]);

  useEffect(() => {
    loadRepoState().catch(() => {});
  }, [loadRepoState]);

  const ghPollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const ghTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const ghCountdownRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const glCountdownRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const glCancelledRef = useRef(false);
  const glConnectingRef = useRef(false);

  useEffect(() => {
    glConnectingRef.current = sections.gitlab.connecting;
  }, [sections.gitlab.connecting]);

  /** Counts one platform's remaining seconds down to zero. */
  function startCountdown(
    platform: PlatformId,
    seconds: number,
    ref: React.RefObject<ReturnType<typeof setInterval> | null>,
  ) {
    setCountdown((prev) => ({ ...prev, [platform]: seconds }));
    ref.current = setInterval(() => {
      setCountdown((prev) => ({
        ...prev,
        [platform]: prev[platform] <= 1 ? 0 : prev[platform] - 1,
      }));
    }, 1000);
  }

  function clearGitHubTimers() {
    for (const ref of [ghPollRef, ghTimeoutRef, ghCountdownRef]) {
      if (ref.current) {
        clearInterval(ref.current as ReturnType<typeof setInterval>);
        ref.current = null;
      }
    }
    setCountdown((prev) => ({ ...prev, github: 0 }));
  }

  function cancelGitHubAuth() {
    clearGitHubTimers();
    update("github", {
      connecting: false,
      deviceCode: null,
      error: { kind: "none" },
    });
  }

  function clearGitLabCountdown() {
    if (glCountdownRef.current) {
      clearInterval(glCountdownRef.current);
      glCountdownRef.current = null;
    }
    setCountdown((prev) => ({ ...prev, gitlab: 0 }));
  }

  function cancelGitLabAuth() {
    glCancelledRef.current = true;
    void api.gitlabOauthAbort().catch(() => {});
    clearGitLabCountdown();
    update("gitlab", { connecting: false, error: { kind: "none" } });
  }

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
      }, COPY_HINT_MS);
    } catch {
      /* the hint simply does not appear */
    }
  }

  function cleanupUnsavedTokens() {
    if (!isEdit) {
      void api.deleteProfileTokens(profileId).catch(() => {});
      return;
    }
    const initial = initialPlatformsRef.current;
    for (const platform of PLATFORMS) {
      if (!initial[platform] && sections[platform].connected) {
        void api.deletePlatformToken({ profileId, platform }).catch(() => {});
      }
    }
  }

  function handleProfileCancel() {
    if (sections.gitlab.connecting) cancelGitLabAuth();
    if (sections.github.connecting || sections.github.deviceCode) {
      cancelGitHubAuth();
    }
    cleanupUnsavedTokens();
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
    api
      .listSshKeys()
      .then(setSshKeys)
      .catch(() => {});
    api
      .getSettings()
      .then(setSettings)
      .catch(() => {});
    return () => {
      if (ghPollRef.current) clearInterval(ghPollRef.current);
      if (ghTimeoutRef.current) clearTimeout(ghTimeoutRef.current);
      if (ghCountdownRef.current) clearInterval(ghCountdownRef.current);
      if (glCountdownRef.current) clearInterval(glCountdownRef.current);
      if (glConnectingRef.current) {
        void api.gitlabOauthAbort().catch(() => {});
      }
    };
  }, []);

  async function handleImportFromGit() {
    try {
      const id = await api.getGitIdentity();
      if (id.name && !name.trim()) setName(id.name);
      if (id.email) setImportedEmail(id.email);
    } catch {
      /* nothing to prefill */
    }
  }

  /** The addresses a freshly connected account offers, in the order the form
   *  should prefer them: the noreply one hides a private address, so it wins. */
  function connectedPatch(user: {
    username: string;
    name?: string;
    email?: string;
    noreply_email?: string;
  }): Partial<PlatformState> {
    const noreply = user.noreply_email || "";
    const pubEmail = user.email || "";
    return {
      connecting: false,
      connected: true,
      deviceCode: null,
      username: user.username,
      gitName: user.name || user.username,
      gitEmail: noreply || pubEmail || importedEmail,
      publicEmail: pubEmail,
      noreplyEmail: noreply,
    };
  }

  async function connectGitHub() {
    if (!settings?.github_client_id) {
      update("github", { error: { kind: "settings" } });
      return;
    }
    update("github", { connecting: true, error: { kind: "none" } });
    try {
      const device = await api.githubOauthStart(settings.github_client_id);
      update("github", { deviceCode: device });
      await openUrl(device.verification_uri);

      const expiresIn = device.expires_in || GITHUB_FALLBACK_EXPIRY_S;
      startCountdown("github", expiresIn, ghCountdownRef);

      ghTimeoutRef.current = setTimeout(() => {
        clearGitHubTimers();
        update("github", {
          connecting: false,
          deviceCode: null,
          error: { kind: "message", text: m.form.authTimedOut },
        });
      }, expiresIn * 1000);

      ghPollRef.current = setInterval(
        async () => {
          try {
            const user = await api.githubOauthPoll({
              clientId: settings.github_client_id,
              deviceCode: device.device_code,
              profileId,
            });
            if (user) {
              clearGitHubTimers();
              update("github", connectedPatch(user));
            }
          } catch (e) {
            clearGitHubTimers();
            update("github", {
              connecting: false,
              deviceCode: null,
              error: { kind: "message", text: String(e) },
            });
          }
        },
        (device.interval + 1) * 1000,
      );
    } catch (e) {
      update("github", {
        connecting: false,
        error: { kind: "message", text: String(e) },
      });
    }
  }

  async function connectGitLab() {
    if (!settings?.gitlab_client_id) {
      update("gitlab", { error: { kind: "settings" } });
      return;
    }
    glCancelledRef.current = false;
    update("gitlab", { connecting: true, error: { kind: "none" } });
    startCountdown("gitlab", GITLAB_TIMEOUT_S, glCountdownRef);

    try {
      const user = await api.gitlabOauthConnect({
        clientId: settings.gitlab_client_id,
        profileId,
      });
      clearGitLabCountdown();
      if (glCancelledRef.current) return;
      update("gitlab", connectedPatch(user));
    } catch (e) {
      clearGitLabCountdown();
      if (glCancelledRef.current) return;
      update("gitlab", {
        connecting: false,
        error: { kind: "message", text: String(e) },
      });
    }
  }

  async function connectBitbucket() {
    if (!bbEmail.trim() || !bbToken.trim()) {
      update("bitbucket", {
        error: { kind: "message", text: m.form.bitbucket.errCreds },
      });
      return;
    }
    update("bitbucket", { connecting: true, error: { kind: "none" } });
    try {
      const user = await api.connectBitbucket({
        profileId,
        email: bbEmail.trim(),
        apiToken: bbToken.trim(),
      });
      update("bitbucket", { ...connectedPatch(user), noreplyEmail: "" });
      setBbToken("");
    } catch (e) {
      update("bitbucket", {
        connecting: false,
        error: { kind: "message", text: String(e) },
      });
    }
  }

  const connectors: Record<PlatformId, () => void> = {
    github: connectGitHub,
    gitlab: connectGitLab,
    bitbucket: connectBitbucket,
  };

  async function generateAndUpload(platform: PlatformId) {
    const section = sections[platform];
    update(platform, { error: { kind: "none" } });
    try {
      const pair = await api.generateAndUploadKey({
        platform,
        profileId,
        username: section.username,
        email: section.gitEmail || "git@git-account-manager",
      });
      update(platform, {
        sshPrivateKeyPath: pair.private_key_path,
        sshPublicKeyPath: pair.public_key_path,
        keyUploaded: true,
      });
      setSshKeys(await api.listSshKeys());
    } catch (e) {
      update(platform, { error: { kind: "message", text: String(e) } });
    }
  }

  async function uploadExistingKey(platform: PlatformId) {
    const section = sections[platform];
    if (!section.sshPublicKeyPath) return;
    update(platform, { error: { kind: "none" } });
    try {
      const keyContent = await api.readPublicKey(section.sshPublicKeyPath);
      await api.uploadSshKeyToPlatform({
        platform,
        profileId,
        title: `git-account-manager: ${name}`,
        keyContent,
      });
      update(platform, { keyUploaded: true });
    } catch (e) {
      update(platform, { error: { kind: "message", text: String(e) } });
    }
  }

  function buildAccount(s: PlatformState): PlatformAccount | undefined {
    if (!s.connected || !s.sshPrivateKeyPath) return undefined;
    return {
      username: s.username,
      git_name: s.gitName,
      git_email: s.gitEmail,
      ssh_private_key_path: s.sshPrivateKeyPath,
      ssh_public_key_path: s.sshPublicKeyPath,
    };
  }

  /** The profile as it stands in the form. The repository scan needs this rather
   *  than the saved copy: an account connected a moment ago is what tells the
   *  evidence ladder which namespaces belong to this profile. */
  function draftProfile(): Profile {
    return {
      id: profileId,
      name: name.trim(),
      default_platform: defaultPlatform,
      github: buildAccount(sections.github),
      gitlab: buildAccount(sections.gitlab),
      bitbucket: buildAccount(sections.bitbucket),
      is_active: profile?.is_active || false,
    };
  }

  async function handleSave() {
    if (!name.trim()) {
      setError(m.form.errProfileName);
      return;
    }
    if (!PLATFORMS.some((p) => sections[p].connected)) {
      setError(m.form.errConnectOne);
      return;
    }
    setSaving(true);
    setError("");

    const p = draftProfile();

    try {
      await api.saveProfile(p);
      const report = await api.applyProfileRepos(
        buildRepoPlan({
          profileId: p.id,
          roots,
          repos,
          selected,
          storedBindings,
        }),
      );
      if (report.failed.length > 0) {
        setError(
          `${fmt(m.repos.applyPartial, {
            bound: report.bound,
            failed: report.failed.length,
          })}\n${report.failed.map((f) => `${f.path}: ${f.error}`).join("\n")}`,
        );
        setSaving(false);
        return;
      }
      onSave(p);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleDisconnect(deleteKeys: boolean) {
    if (!disconnectTarget) return;
    const { platform, keyPath, pubKeyPath } = disconnectTarget;

    if (deleteKeys && keyPath) {
      if (pubKeyPath) {
        await api
          .removeSshKeyFromPlatform({
            platform,
            profileId,
            publicKeyPath: pubKeyPath,
          })
          .catch(() => {});
      }
      await api.deleteSshKeys([keyPath]).catch(() => {});
    }
    await api.deletePlatformToken({ profileId, platform }).catch(() => {});
    update(platform, emptyPlatform());
    setDisconnectTarget(null);

    if (!profile) return;

    // Disconnecting the last account leaves nothing to identify the profile
    // with, so the profile itself goes rather than becoming an empty shell.
    const remaining = PLATFORMS.filter(
      (p) => p !== platform && sections[p].connected,
    );
    if (remaining.length === 0) {
      onDelete(profile.id, false);
      return;
    }

    const updated: Profile = { ...profile, [platform]: undefined };
    try {
      await api.saveProfile(updated);
      onSave(updated);
    } catch {
      /* keep the form open so the failure is visible */
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

  const connectedPlatforms = PLATFORMS.filter((p) => sections[p].connected);
  const activePlatform =
    connectedPlatforms.find((p) => p === defaultPlatform) ??
    connectedPlatforms[0];
  const activeSection = activePlatform ? sections[activePlatform] : undefined;

  const cancelAuthFor: Partial<Record<PlatformId, () => void>> = {
    github: cancelGitHubAuth,
    gitlab: cancelGitLabAuth,
  };

  return (
    <>
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between border-b border-bd px-6 py-4">
          <h2 className="text-lg font-semibold text-fg">
            {isEdit ? m.form.editTitle : m.form.newTitle}
          </h2>
          <button
            type="button"
            onClick={handleProfileCancel}
            title={m.form.cancel}
            className="text-fg-4 hover:text-fg-2"
          >
            <CloseIcon />
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto p-6">
          <div>
            <label
              htmlFor="profile-name"
              className="mb-1 block text-sm font-medium text-fg-3"
            >
              {m.form.profileName}
            </label>
            <input
              id="profile-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={m.form.profileNamePlaceholder}
              className="field"
            />
          </div>

          {!isEdit && (
            <button
              type="button"
              onClick={handleImportFromGit}
              className="text-xs text-link hover:text-link-hover hover:underline"
            >
              {m.form.importFromGit}
            </button>
          )}

          {PLATFORMS.map((platform) => (
            <PlatformSection
              key={platform}
              platform={platform}
              state={sections[platform]}
              onChange={(patch) => update(platform, patch)}
              onConnect={connectors[platform]}
              onCancelAuth={cancelAuthFor[platform]}
              countdown={countdown[platform]}
              credentials={
                platform === "bitbucket"
                  ? {
                      email: bbEmail,
                      token: bbToken,
                      setEmail: setBbEmail,
                      setToken: setBbToken,
                    }
                  : undefined
              }
              sshKeys={sshKeys}
              importedEmail={importedEmail}
              copiedPublicPath={copiedPublicPath}
              onCopyPublicKey={handleCopyPublicKey}
              onGenerateKey={() => generateAndUpload(platform)}
              onUploadExistingKey={() => uploadExistingKey(platform)}
              onSelectKey={(key) =>
                update(platform, {
                  selectedKey: key.private_key_path,
                  sshPrivateKeyPath: key.private_key_path,
                  sshPublicKeyPath: key.public_key_path,
                })
              }
              onDisconnect={() =>
                setDisconnectTarget({
                  platform,
                  keyPath: sections[platform].sshPrivateKeyPath,
                  pubKeyPath: sections[platform].sshPublicKeyPath,
                })
              }
              onOpenSettings={onSettings}
            />
          ))}

          {connectedPlatforms.length >= 2 && (
            <div className="panel">
              <p className="mb-1 block text-sm font-medium text-fg-3">
                {m.form.defaultIdentity}
              </p>
              <p className="mb-1 text-xs text-fg-5">
                {rich(m.form.defaultIdentityHint1, { codeClass: "text-fg-4" })}
              </p>
              <p className="mb-3 text-xs text-fg-5">
                {m.form.defaultIdentityHint2}
              </p>
              <div className="mb-3 flex flex-wrap gap-3">
                {connectedPlatforms.map((p) => (
                  <button
                    type="button"
                    key={p}
                    onClick={() => setDefaultPlatform(p)}
                    className={`rounded-md px-3 py-1.5 text-sm ${
                      defaultPlatform === p
                        ? "bg-blue-600 text-white"
                        : "bg-subtle text-fg-3"
                    }`}
                  >
                    {PLATFORM_LABEL[p]}
                  </button>
                ))}
              </div>
              {activeSection && (
                <p className="text-xs text-fg-4">
                  {m.form.activeLabel}{" "}
                  <span className="font-medium text-fg-2">
                    {activeSection.gitName}
                  </span>{" "}
                  <span className="text-fg-5">
                    &lt;{activeSection.gitEmail}&gt;
                  </span>
                </p>
              )}
            </div>
          )}

          {connectedPlatforms.length > 0 && (
            <ProfileRepos
              profile={draftProfile()}
              platforms={connectedPlatforms}
              roots={roots}
              setRoots={setRoots}
              repos={repos}
              setRepos={setRepos}
              selected={selected}
              setSelected={setSelected}
              statuses={statuses}
              loading={reposLoading}
              onFixed={() => loadRepoState().catch(() => {})}
            />
          )}

          {error && (
            <div className="rounded-md bg-danger-bg p-3 text-sm whitespace-pre-wrap text-danger-fg">
              {error}
            </div>
          )}
        </div>

        <div className="flex gap-3 border-t border-bd px-6 py-4">
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="btn-primary inline-flex items-center gap-2"
          >
            {saving && <Spinner />}
            {saving
              ? m.form.saving
              : isEdit
                ? m.form.saveChanges
                : m.form.createProfile}
          </button>
          <button
            type="button"
            onClick={handleProfileCancel}
            className="btn-subtle"
          >
            {m.form.cancel}
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={disconnectTarget !== null}
        title={fmt(m.form.disconnectTitle, {
          platform: PLATFORM_LABEL[disconnectTarget?.platform ?? "github"],
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
