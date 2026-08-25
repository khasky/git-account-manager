export interface PlatformAccount {
  username: string;
  git_name: string;
  git_email: string;
  ssh_private_key_path: string;
  ssh_public_key_path: string;
}

export interface Profile {
  id: string;
  name: string;
  default_platform?: string;
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
}

export interface PlatformUser {
  username: string;
  name?: string;
  email?: string;
  noreply_email?: string;
  avatar_url?: string;
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
  sshExe: string | null;
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

export type PlatformId = "github" | "gitlab" | "bitbucket";

/** A folder whose repositories belong to one profile. */
export interface RepoRoot {
  path: string;
  profile_id: string;
  platform: string;
}

/** One repository pinned to a profile. */
export interface RepoBinding {
  path: string;
  profile_id: string;
  platform: string;
  pin_remote_alias: boolean;
  install_hook: boolean;
  extra_allowed_emails: string[];
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
  remote_url: string;
  host: string;
  owner: string;
  repo: string;
  suggested_profile_id: string | null;
  suggested_platform: string | null;
  reason: "alias" | "owner" | "ambiguous" | "unknown";
  candidate_profile_ids: string[];
  bound: boolean;
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
}

export interface RepoStatus {
  path: string;
  name: string;
  profile_id: string;
  profile_name: string;
  platform: string;
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

export interface RepoAccess {
  found: boolean;
  can_push: boolean;
  full_name: string;
}
