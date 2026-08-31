import { openUrl } from "@tauri-apps/plugin-opener";
import { fmt, rich, useI18n } from "../i18n";
import { PLATFORM_LABEL, profileUrl } from "../platforms";
import type { DeviceCodeResponse, PlatformId, SshKeyInfo } from "../types";
import { CheckIcon, CopyIcon } from "./icons";

/**
 * Why a platform's card is showing an error.
 *
 * This used to be one free-text string carrying the sentinel value
 * `"settings_required"`, which the renderer compared against to decide whether
 * to draw a link to Settings — a string that had to match in two files and
 * could never be told apart from a backend message that happened to say the
 * same thing. The shape now says which case it is.
 */
export type SectionError =
  | { kind: "none" }
  | { kind: "settings" }
  | { kind: "message"; text: string };

export interface PlatformState {
  connected: boolean;
  connecting: boolean;
  username: string;
  gitName: string;
  gitEmail: string;
  publicEmail: string;
  noreplyEmail: string;
  sshPrivateKeyPath: string;
  sshPublicKeyPath: string;
  sshSource: "existing" | "generate";
  selectedKey: string;
  error: SectionError;
  keyUploaded: boolean;
  signCommits: boolean;
  /** Why the key this account uses is not registered for signing. */
  signingError: string;
  deviceCode: DeviceCodeResponse | null;
}

export function emptyPlatform(): PlatformState {
  return {
    connected: false,
    connecting: false,
    username: "",
    gitName: "",
    gitEmail: "",
    publicEmail: "",
    noreplyEmail: "",
    sshPrivateKeyPath: "",
    sshPublicKeyPath: "",
    sshSource: "generate",
    selectedKey: "",
    error: { kind: "none" },
    keyUploaded: false,
    // On for anything connected from here on: a signed commit is what earns the
    // "Verified" badge, and a key that can sign costs nothing extra to make.
    signCommits: true,
    signingError: "",
    deviceCode: null,
  };
}

interface Props {
  platform: PlatformId;
  state: PlatformState;
  onChange: (patch: Partial<PlatformState>) => void;
  onConnect: () => void;
  /** Present only while a flow this app can interrupt is running. */
  onCancelAuth?: () => void;
  /** Seconds left in the running flow; 0 hides the counter. */
  countdown: number;
  /** Bitbucket authenticates with a pasted email + API token, not OAuth. */
  credentials?: {
    email: string;
    token: string;
    setEmail: (v: string) => void;
    setToken: (v: string) => void;
  };
  sshKeys: SshKeyInfo[];
  importedEmail: string;
  copiedPublicPath: string | null;
  onCopyPublicKey: (path: string) => void;
  onGenerateKey: () => void;
  onUploadExistingKey: () => void;
  onSelectKey: (key: SshKeyInfo) => void;
  onDisconnect: () => void;
  onOpenSettings: () => void;
}

/** One of the addresses the platform offers, as a pickable row. */
function EmailChoice({
  email,
  badge,
  badgeClass,
  selected,
  onPick,
}: {
  email: string;
  badge: string;
  badgeClass: string;
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onPick}
      className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-xs transition-colors ${
        selected
          ? "border-selected-border bg-selected-bg text-selected-fg"
          : "border-bd-s bg-input text-fg-3 hover:border-bd-s"
      }`}
    >
      <span
        className={`shrink-0 rounded px-1 py-0.5 text-[10px] font-medium ${badgeClass}`}
      >
        {badge}
      </span>
      <span className="truncate">{email}</span>
    </button>
  );
}

export default function PlatformSection({
  platform,
  state,
  onChange,
  onConnect,
  onCancelAuth,
  countdown,
  credentials,
  sshKeys,
  importedEmail,
  copiedPublicPath,
  onCopyPublicKey,
  onGenerateKey,
  onUploadExistingKey,
  onSelectKey,
  onDisconnect,
  onOpenSettings,
}: Props) {
  const { m } = useI18n();
  const label = PLATFORM_LABEL[platform];
  const clock = `${Math.floor(countdown / 60)}:${String(countdown % 60).padStart(2, "0")}`;

  function renderError() {
    if (state.error.kind === "none") return null;
    if (state.error.kind === "settings") {
      return (
        <p className="text-xs text-danger-fg">
          {rich(fmt(m.form.errSettingsRequired, { platform: label }), {
            onLink: onOpenSettings,
          })}
        </p>
      );
    }
    return <p className="text-xs text-danger-fg">{state.error.text}</p>;
  }

  /** Bitbucket offers no noreply address, so the address that hides a private
   *  one has to be set up on the account first. Shown before connecting and
   *  again next to the email field, which is where its absence is noticed. */
  function renderEmailPrivacy() {
    if (platform !== "bitbucket") return null;
    return (
      <p className="text-xs text-fg-5">
        {rich(m.form.bitbucket.emailPrivacy, {
          onLink: () =>
            openUrl("https://bitbucket.org/account/settings/email/"),
        })}
      </p>
    );
  }

  function renderConnect() {
    if (credentials) {
      return (
        <div className="space-y-2">
          <p className="text-xs text-fg-5">
            {rich(m.form.bitbucket.help, {
              onLink: () =>
                openUrl(
                  "https://id.atlassian.com/manage-profile/security/api-tokens",
                ),
            })}
          </p>
          <input
            type="text"
            aria-label={m.form.bitbucket.emailPlaceholder}
            value={credentials.email}
            onChange={(e) => credentials.setEmail(e.target.value)}
            placeholder={m.form.bitbucket.emailPlaceholder}
            className="field-sm"
          />
          <input
            type="password"
            aria-label={m.form.bitbucket.tokenPlaceholder}
            value={credentials.token}
            onChange={(e) => credentials.setToken(e.target.value)}
            placeholder={m.form.bitbucket.tokenPlaceholder}
            className="field-sm"
          />
          <p className="text-xs text-fg-5">{m.form.bitbucket.scopesHint}</p>
          {renderEmailPrivacy()}
          <button
            type="button"
            onClick={onConnect}
            disabled={state.connecting}
            className="w-full rounded-md bg-blue-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:opacity-50"
          >
            {state.connecting
              ? m.form.waitingAuth
              : fmt(m.form.connectWith, { platform: label })}
          </button>
          {renderError()}
        </div>
      );
    }

    return (
      <div className="space-y-3">
        {state.deviceCode ? (
          <div className="space-y-2 rounded-md border border-info-border bg-info-bg p-3">
            <p className="text-sm text-fg-3">{m.form.enterCode}</p>
            <p className="font-mono text-2xl font-bold tracking-widest text-link">
              {state.deviceCode.user_code}
            </p>
            <div className="flex items-center justify-between">
              <p className="text-xs text-fg-4">
                {m.form.waitingAuth}
                {countdown > 0 && (
                  <span className="ml-1 text-fg-5">({clock})</span>
                )}
              </p>
              {onCancelAuth && (
                <button
                  type="button"
                  onClick={onCancelAuth}
                  className="rounded-md bg-subtle px-3 py-1 text-xs text-fg-3 transition-colors hover:bg-hover hover:text-fg"
                >
                  {m.form.cancel}
                </button>
              )}
            </div>
          </div>
        ) : state.connecting ? (
          <div className="space-y-2 rounded-md border border-info-border bg-info-bg p-3">
            <div className="flex items-center justify-between">
              <p className="text-xs text-fg-4">
                {m.form.waitingBrowser}
                {countdown > 0 && (
                  <span className="ml-1 text-fg-5">({clock})</span>
                )}
              </p>
              {onCancelAuth && (
                <button
                  type="button"
                  onClick={onCancelAuth}
                  className="rounded-md bg-subtle px-3 py-1 text-xs text-fg-3 transition-colors hover:bg-hover hover:text-fg"
                >
                  {m.form.cancel}
                </button>
              )}
            </div>
            {platform === "gitlab" && (
              <p className="text-xs text-fg-5">{m.form.gitlabClipboardHint}</p>
            )}
          </div>
        ) : (
          <button
            type="button"
            onClick={onConnect}
            className="w-full rounded-md bg-blue-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-blue-500"
          >
            {fmt(m.form.connectWith, { platform: label })}
          </button>
        )}
        {renderError()}
      </div>
    );
  }

  function renderKeySection() {
    if (state.sshPrivateKeyPath && state.keyUploaded) {
      return (
        <div className="flex flex-col gap-2 rounded-md border border-active-border bg-active-bg px-3 py-2 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-center gap-2">
            <CheckIcon className="h-4 w-4 shrink-0 text-success-icon" />
            <span className="text-xs text-success-fg">
              {fmt(m.form.uploadedTo, {
                file: state.sshPrivateKeyPath.split(/[\\/]/).pop() || "",
                platform: label,
              })}
            </span>
          </div>
          {state.sshPublicKeyPath ? (
            <button
              type="button"
              onClick={() => onCopyPublicKey(state.sshPublicKeyPath)}
              className="inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md border border-bd-s bg-input px-2.5 py-1 text-xs font-medium text-fg-2 transition-colors hover:bg-hover"
            >
              <CopyIcon />
              {copiedPublicPath === state.sshPublicKeyPath
                ? m.form.copied
                : m.form.copyPublicKey}
            </button>
          ) : null}
        </div>
      );
    }

    return (
      <div className="space-y-2">
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => onChange({ sshSource: "generate" })}
            className={`rounded-md px-2 py-1 text-xs ${state.sshSource === "generate" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
          >
            {m.form.generateUpload}
          </button>
          <button
            type="button"
            onClick={() => onChange({ sshSource: "existing" })}
            className={`rounded-md px-2 py-1 text-xs ${state.sshSource === "existing" ? "bg-blue-600 text-white" : "bg-subtle text-fg-3"}`}
          >
            {m.form.useExisting}
          </button>
        </div>

        {state.sshSource === "generate" ? (
          <button
            type="button"
            onClick={onGenerateKey}
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
                  <div key={k.private_key_path} className="flex gap-1">
                    <button
                      type="button"
                      onClick={() => onSelectKey(k)}
                      className={`min-w-0 flex-1 rounded-md border px-2 py-1.5 text-left text-xs transition-colors ${
                        state.selectedKey === k.private_key_path
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
                        onCopyPublicKey(k.public_key_path);
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
            {state.sshPrivateKeyPath && !state.keyUploaded && (
              <button
                type="button"
                onClick={onUploadExistingKey}
                className="rounded-md bg-subtle px-3 py-1.5 text-xs text-fg-2 hover:bg-hover"
              >
                {fmt(m.form.uploadTo, { platform: label })}
              </button>
            )}
          </div>
        )}
      </div>
    );
  }

  const nameId = `git-name-${platform}`;
  const emailId = `git-email-${platform}`;
  const signId = `sign-commits-${platform}`;
  const hasChoices = state.noreplyEmail || state.publicEmail || importedEmail;

  return (
    <div className="panel">
      <div className="mb-3 flex items-center justify-between">
        <h4 className="font-medium text-fg-2">{label}</h4>
        {state.connected && (
          <button
            type="button"
            onClick={() => openUrl(profileUrl(platform, state.username))}
            className="text-sm text-link hover:text-link-hover hover:underline"
          >
            @{state.username}
          </button>
        )}
      </div>

      {!state.connected ? (
        renderConnect()
      ) : (
        <div className="space-y-3">
          <div>
            <label htmlFor={nameId} className="mb-1 block text-xs text-fg-4">
              {m.form.gitName}
            </label>
            <input
              id={nameId}
              type="text"
              value={state.gitName}
              onChange={(e) => onChange({ gitName: e.target.value })}
              className="field-sm"
            />
          </div>

          <div>
            <label htmlFor={emailId} className="mb-1 block text-xs text-fg-4">
              {m.form.gitEmail}
            </label>
            {hasChoices ? (
              <div className="space-y-1.5">
                {state.noreplyEmail && (
                  <EmailChoice
                    email={state.noreplyEmail}
                    badge={m.form.noreplyBadge}
                    badgeClass="bg-badge-ok-bg text-badge-ok-fg"
                    selected={state.gitEmail === state.noreplyEmail}
                    onPick={() => onChange({ gitEmail: state.noreplyEmail })}
                  />
                )}
                {state.publicEmail &&
                  state.publicEmail !== state.noreplyEmail && (
                    <EmailChoice
                      email={state.publicEmail}
                      badge={m.form.publicBadge}
                      badgeClass="bg-subtle text-fg-3"
                      selected={state.gitEmail === state.publicEmail}
                      onPick={() => onChange({ gitEmail: state.publicEmail })}
                    />
                  )}
                {importedEmail &&
                  importedEmail !== state.noreplyEmail &&
                  importedEmail !== state.publicEmail && (
                    <EmailChoice
                      email={importedEmail}
                      badge="git config"
                      badgeClass="bg-subtle text-fg-3"
                      selected={state.gitEmail === importedEmail}
                      onPick={() => onChange({ gitEmail: importedEmail })}
                    />
                  )}
                <input
                  id={emailId}
                  type="text"
                  value={state.gitEmail}
                  onChange={(e) => onChange({ gitEmail: e.target.value })}
                  placeholder={m.form.customEmailPlaceholder}
                  className="w-full rounded-md border border-bd-s bg-input px-2.5 py-1.5 text-xs text-fg outline-none focus:border-blue-500"
                />
              </div>
            ) : (
              <input
                id={emailId}
                type="text"
                value={state.gitEmail}
                onChange={(e) => onChange({ gitEmail: e.target.value })}
                className="field-sm"
              />
            )}
            <div className="mt-1.5">{renderEmailPrivacy()}</div>
          </div>

          <div>
            <p className="mb-1 block text-xs text-fg-4">{m.form.sshKey}</p>
            {renderKeySection()}
          </div>

          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <label
                htmlFor={signId}
                className="block text-xs font-medium text-fg-3"
              >
                {m.form.signCommits}
              </label>
              <p className="mt-0.5 text-xs text-fg-5">
                {m.form.signCommitsHint}
              </p>
              {state.signingError && (
                <p className="mt-1 text-xs text-danger-fg">
                  {fmt(m.form.signingFailed, {
                    error: state.signingError,
                    platform: label,
                  })}
                </p>
              )}
            </div>
            <input
              id={signId}
              type="checkbox"
              checked={state.signCommits}
              onChange={(e) => onChange({ signCommits: e.target.checked })}
              className="mt-0.5 h-4 w-4 shrink-0 accent-emerald-600"
            />
          </div>

          {renderError()}

          <button
            type="button"
            onClick={onDisconnect}
            className="text-xs text-danger-fg hover:underline"
          >
            {m.form.disconnect}
          </button>
        </div>
      )}
    </div>
  );
}
