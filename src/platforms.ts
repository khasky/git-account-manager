import type { PlatformId } from "./types";

/** The platforms this app knows, in the order every list and menu shows them. */
export const PLATFORMS: readonly PlatformId[] = [
  "github",
  "gitlab",
  "bitbucket",
  "azure",
];

export const PLATFORM_LABEL: Record<PlatformId, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
  azure: "Azure DevOps",
};

const PROFILE_URL_BASE: Record<PlatformId, string> = {
  github: "https://github.com/",
  gitlab: "https://gitlab.com/",
  bitbucket: "https://bitbucket.org/",
  // An Azure DevOps account here is an organization, so this is its home.
  azure: "https://dev.azure.com/",
};

export function profileUrl(platform: PlatformId, username: string): string {
  return PROFILE_URL_BASE[platform] + username;
}

/** Where each platform issues the token git authenticates HTTPS remotes with.
 *  Azure DevOps issues tokens per organization; without one, `dev.azure.com`
 *  opens the organization the user last visited. */
export function tokenPageUrl(platform: PlatformId, username: string): string {
  switch (platform) {
    case "github":
      return "https://github.com/settings/tokens";
    case "gitlab":
      return "https://gitlab.com/-/user_settings/personal_access_tokens";
    case "bitbucket":
      return "https://id.atlassian.com/manage-profile/security/api-tokens";
    case "azure":
      return username
        ? `https://dev.azure.com/${encodeURIComponent(username)}/_usersSettings/tokens`
        : "https://dev.azure.com/";
  }
}

/** Azure DevOps keeps SSH keys per organization, and a token cannot add one,
 *  so this is where the user pastes it. */
export function azureKeysPageUrl(organization: string): string {
  return `https://dev.azure.com/${encodeURIComponent(organization)}/_usersSettings/keys`;
}
