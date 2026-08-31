/**
 * The whole backend surface, in one place.
 *
 * Every call used to be a bare `invoke("some_command", { camelCased: ... })`.
 * A typo in either the command name or an argument key is invisible to the
 * compiler and only shows up as a rejected promise at runtime, on a path a user
 * has to reach by hand — connecting an account, saving a profile. Naming each
 * command once here means the rest of the app calls functions, and a rename on
 * the Rust side breaks the build instead of the app.
 *
 * Argument keys are camelCase because that is what Tauri converts a command's
 * snake_case parameters to; the payload *values* keep the snake_case field
 * names serde expects.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyReport,
  BindResult,
  DeviceCodeResponse,
  DiscoveredRepo,
  DoctorReport,
  GitIdentity,
  GuardSettings,
  OAuthSettings,
  OpenSshIntegrationProbe,
  PlatformId,
  PlatformUser,
  Profile,
  RepoPlan,
  RepoReach,
  RepoRoot,
  RepoState,
  SshKeyInfo,
  SshKeyPair,
} from "./types";

// -- profiles ---------------------------------------------------------------

export const getProfiles = () => invoke<Profile[]>("get_profiles");

export const saveProfile = (profile: Profile) =>
  invoke<void>("save_profile", { profile });

export const deleteProfile = (id: string) =>
  invoke<void>("delete_profile", { id });

export const activateProfile = (id: string) =>
  invoke<void>("activate_profile", { id });

export const getGitIdentity = () => invoke<GitIdentity>("get_git_identity");

// -- SSH keys ---------------------------------------------------------------

export const listSshKeys = () => invoke<SshKeyInfo[]>("list_ssh_keys");

export const readPublicKey = (path: string) =>
  invoke<string>("read_public_key", { path });

export const deleteSshKeys = (paths: string[]) =>
  invoke<void>("delete_ssh_keys", { paths });

export const generateAndUploadKey = (args: {
  platform: PlatformId;
  profileId: string;
  username: string;
  email: string;
  sign: boolean;
}) => invoke<SshKeyPair>("generate_and_upload_key", args);

/** Resolves to the reason signing could not be enabled, or null when the key is
 *  registered for everything that was asked of it. */
export const uploadSshKeyToPlatform = (args: {
  platform: PlatformId;
  profileId: string;
  title: string;
  keyContent: string;
  sign: boolean;
}) => invoke<string | null>("upload_ssh_key_to_platform", args);

export const removeSshKeyFromPlatform = (args: {
  platform: PlatformId;
  profileId: string;
  publicKeyPath: string;
}) => invoke<void>("remove_ssh_key_from_platform", args);

// -- accounts ---------------------------------------------------------------

export const connectBitbucket = (args: {
  profileId: string;
  email: string;
  apiToken: string;
}) => invoke<PlatformUser>("connect_bitbucket", args);

export const githubOauthStart = (clientId: string) =>
  invoke<DeviceCodeResponse>("github_oauth_start", { clientId });

/** Resolves to null while the user has not finished authorizing yet. */
export const githubOauthPoll = (args: {
  clientId: string;
  deviceCode: string;
  profileId: string;
}) => invoke<PlatformUser | null>("github_oauth_poll", args);

export const gitlabOauthConnect = (args: {
  clientId: string;
  profileId: string;
}) => invoke<PlatformUser>("gitlab_oauth_connect", args);

export const gitlabOauthAbort = () => invoke<void>("gitlab_oauth_abort");

export const deletePlatformToken = (args: {
  profileId: string;
  platform: PlatformId;
}) => invoke<void>("delete_platform_token", args);

export const deleteProfileTokens = (profileId: string) =>
  invoke<void>("delete_profile_tokens", { profileId });

// -- settings ---------------------------------------------------------------

export const getSettings = () => invoke<OAuthSettings>("get_settings");

export const saveSettings = (settings: OAuthSettings) =>
  invoke<void>("save_settings", { settings });

export const openSshIntegrationProbe = () =>
  invoke<OpenSshIntegrationProbe>("openssh_integration_probe");

export const saveGuardSettings = (settings: GuardSettings) =>
  invoke<void>("save_guard_settings", { settings });

// -- repositories -----------------------------------------------------------

export const getRepoState = () => invoke<RepoState>("get_repo_state");

export const doctor = () => invoke<DoctorReport>("doctor");

/** Scans against the profile as it stands in the form, which may not be saved
 *  yet — that is what lets a new profile configure its folders before it
 *  exists on disk. */
export const scanProfileRepositories = (args: {
  profile: Profile;
  roots: RepoRoot[];
}) => invoke<DiscoveredRepo[]>("scan_profile_repositories", args);

export const applyProfileRepos = (plan: RepoPlan) =>
  invoke<ApplyReport>("apply_profile_repos", { plan });

export const fixRepository = (path: string) =>
  invoke<BindResult>("fix_repository", { path });

export const allowEmailInRepository = (args: { path: string; email: string }) =>
  invoke<BindResult>("allow_email_in_repository", args);

export const verifyRepoAccess = (args: {
  profileId: string;
  platform: PlatformId;
  owner: string;
  repo: string;
}) => invoke<RepoReach>("verify_repo_access", args);

export const probeSshAlias = (host: string) =>
  invoke<string>("probe_ssh_alias", { host });

// -- tray -------------------------------------------------------------------

export const setTrayLabels = (args: {
  show: string;
  quit: string;
  activePrefix: string;
  noActive: string;
}) => invoke<void>("set_tray_labels", args);
