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
  /** Run `gh auth switch` to the active profile's GitHub login on every switch. */
  switch_gh_account: boolean;
  /** Answer git's credential requests on HTTPS remotes from the active profile. */
  use_https_credential_helper: boolean;
}

export interface GhProbe {
  available: boolean;
  logins: string[];
  active: string | null;
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

/** A folder whose repositories all belong to one profile's account. The rule is
 *  the folder: one generated `includeIf` block covers everything under it. */
export interface RepoRoot {
  path: string;
  profile_id: string;
  platform: PlatformId;
  /** `owner/repo` of what was inside when the folder was last scanned. Owned by
   *  the backend, which records it on save so a folder that moves is found. */
  fingerprint: string[];
}

export interface GuardSettings {
  /** Leave the machine without a default identity, so a repository no folder
   *  rule claims refuses to commit instead of borrowing the active profile. */
  unset_global_identity: boolean;
  /** Route hooks through the app's global dispatchers so commits are checked. */
  guard_commits: boolean;
}

export interface RepoState {
  roots: RepoRoot[];
  guard: GuardSettings;
}

/** One repository a folder rule covers. Read-only: the rule reaches all of them
 *  equally, so the list is there to be seen, not chosen from. */
export interface FolderRepo {
  path: string;
  /** Where it sits under the folder, which is how the hierarchy is drawn. */
  relative: string;
  name: string;
  root_path: string;
  remote_url: string;
  full_name: string;
  /** Its remote points at a different site than the folder's platform. */
  foreign_host: boolean;
}

export interface SaveFoldersReport {
  folders: number;
  repos: number;
}

export interface FolderCheck {
  id: "exists" | "rule" | "identity" | "local" | "guard";
  ok: boolean;
  detail: string;
}

/** One repository that does not follow its folder's rule. */
export interface RepoIssue {
  path: string;
  relative: string;
  detail: string;
}

export interface FolderStatus {
  path: string;
  profile_id: string;
  profile_name: string;
  platform: PlatformId;
  expected_email: string;
  repos: number;
  checks: FolderCheck[];
  overrides: RepoIssue[];
  bypassed: RepoIssue[];
  moved_to: string | null;
  ok: boolean;
}

/** The cheap check the window runs on a timer. */
export interface FolderWatch {
  path: string;
  profile_id: string;
  profile_name: string;
  state: "ok" | "missing" | "no-rule" | "guard-off";
  /** Filled in by the window, from `locateFolder`, only for what it shows. */
  moved_to?: string | null;
}

/** A result addressed to the control that produced it, so it can be rendered
 *  next to that control instead of at the bottom of the panel. */
export interface RepoNote {
  key: string;
  tone: "ok" | "bad";
  text: string;
}

export interface GuardStatus {
  global_name: string;
  global_email: string;
  gitconfig_path: string;
  rules_written: boolean;
  /** Whose key answers on each bare host; `profile` is null where none does. */
  ssh_hosts: { host: string; profile: string | null }[];
  hooks_path: string | null;
  hooks: "global" | "foreign" | "off";
  use_config_only: boolean;
  ok: boolean;
}

export interface DoctorReport {
  guard: GuardStatus;
  folders: FolderStatus[];
}

/** How far a profile save has got, reported by the backend while it runs. */
export interface SaveProgress {
  stage: "scan" | "apply";
  folder: string;
  folder_index: number;
  folder_count: number;
  repos_done: number;
  repos_total: number;
}
