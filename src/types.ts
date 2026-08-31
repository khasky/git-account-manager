/** The platforms this app knows. Mirrors `models::Platform` in the backend,
 *  whose serialized form is exactly these three strings. */
export type PlatformId = "github" | "gitlab" | "bitbucket";

export interface PlatformAccount {
  username: string;
  git_name: string;
  git_email: string;
  ssh_private_key_path: string;
  ssh_public_key_path: string;
  /** Sign this account's commits with its own SSH key. Absent in profiles saved
   *  before signing existed, which the backend reads as off. */
  sign_commits?: boolean;
}

export interface Profile {
  id: string;
  name: string;
  default_platform?: PlatformId;
  github?: PlatformAccount;
  gitlab?: PlatformAccount;
  bitbucket?: PlatformAccount;
  is_active: boolean;
}

export interface SshKeyInfo {
  name: string;
  private_key_path: string;
  public_key_path: string;
}

export interface SshKeyPair {
  private_key_path: string;
  public_key_path: string;
  /** Set when the key was created and uploaded but could not be registered for
   *  signing — the key works, the "Verified" badge will not appear. */
  signing_error?: string;
}

export interface PlatformUser {
  username: string;
  name?: string;
  email?: string;
  noreply_email?: string;
  avatar_url?: string;
  /** Present when `username` is a fallback the platform could not confirm. */
  username_notice?: string;
}

export interface OAuthSettings {
  github_client_id: string;
  gitlab_client_id: string;
  /** Windows: use OpenSSH for TortoiseGit + Git CLI (registry + core.sshCommand). */
  use_openssh_for_git_tools: boolean;
}

/** Result of `openssh_integration_probe` — Windows-only integration. */
export interface OpenSshIntegrationProbe {
  available: boolean;
  ssh_exe: string | null;
}

export interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface GitIdentity {
  name: string;
  email: string;
}

/** A folder whose repositories belong to one profile. */
export interface RepoRoot {
  path: string;
  profile_id: string;
  platform: PlatformId;
  /** Defaults every repository in this folder starts with. */
  install_hook: boolean;
  pin_remote_alias: boolean;
}

/** One repository pinned to a profile. */
export interface RepoBinding {
  path: string;
  profile_id: string;
  platform: PlatformId;
  pin_remote_alias: boolean;
  install_hook: boolean;
  extra_allowed_emails: string[];
  /** Deliberately set apart from its folder's defaults. */
  overrides_root: boolean;
  /** What `origin` was before the alias replaced it. Owned by the backend,
   *  which carries it across saves so unpinning restores the real address. */
  original_remote_url?: string | null;
}

export interface GuardSettings {
  unset_global_identity: boolean;
  manage_gitconfig_includes: boolean;
  own_bare_ssh_hosts: boolean;
}

export interface RepoState {
  roots: RepoRoot[];
  bindings: RepoBinding[];
  guard: GuardSettings;
}

/** `reason` records how the profile was inferred, never a silent guess. */
export interface DiscoveredRepo {
  path: string;
  name: string;
  root_path: string;
  remote_url: string;
  host: string;
  owner: string;
  repo: string;
  suggested_profile_id: string | null;
  suggested_platform: PlatformId | null;
  reason: "alias" | "owner" | "ambiguous" | "unknown";
  candidate_profile_ids: string[];
  bound: boolean;
  /** Folder defaults already applied by the backend. */
  install_hook: boolean;
  pin_remote_alias: boolean;
  overrides_root: boolean;
}

/** Everything the profile form collected, applied when the profile is saved. */
export interface RepoPlan {
  profile_id: string;
  roots: RepoRoot[];
  bindings: RepoBinding[];
  released: string[];
}

export interface ApplyReport {
  bound: number;
  released: number;
  failed: { path: string; error: string }[];
}

export interface BindResult {
  identity: string;
  remote_url: string | null;
  hook: "installed" | "kept-existing" | "unavailable" | "off";
}

export interface RepoCheck {
  id: "exists" | "identity" | "local" | "remote" | "history" | "hooks";
  ok: boolean;
  detail: string;
  /** Where to look — empty when the detail already says everything. */
  hint: string;
}

/** A result addressed to the control that produced it, so it can be rendered
 *  next to that control instead of at the bottom of the panel. */
export interface RepoNote {
  key: string;
  tone: "ok" | "bad";
  text: string;
}

export interface RepoStatus {
  path: string;
  name: string;
  profile_id: string;
  profile_name: string;
  platform: PlatformId;
  expected_email: string;
  effective_email: string;
  remote_url: string;
  offending_emails: string[];
  checks: RepoCheck[];
  ok: boolean;
}

export interface GuardStatus {
  global_name: string;
  global_email: string;
  use_config_only: boolean;
  includes_managed: boolean;
  gitconfig_path: string;
  ok: boolean;
}

export interface DoctorReport {
  guard: GuardStatus;
  repos: RepoStatus[];
}

export interface RepoReach {
  reachable: boolean;
  full_name: string;
  detail: string;
}
