import type { PlatformId } from "./types";

/** The platforms this app knows, in the order every list and menu shows them. */
export const PLATFORMS: readonly PlatformId[] = [
  "github",
  "gitlab",
  "bitbucket",
];

export const PLATFORM_LABEL: Record<PlatformId, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
};

const PROFILE_URL_BASE: Record<PlatformId, string> = {
  github: "https://github.com/",
  gitlab: "https://gitlab.com/",
  bitbucket: "https://bitbucket.org/",
};

export function profileUrl(platform: PlatformId, username: string): string {
  return PROFILE_URL_BASE[platform] + username;
}

/** Where each platform issues the token git authenticates HTTPS remotes with. */
export const TOKEN_PAGE_URL: Record<PlatformId, string> = {
  github: "https://github.com/settings/tokens",
  gitlab: "https://gitlab.com/-/user_settings/personal_access_tokens",
  bitbucket: "https://id.atlassian.com/manage-profile/security/api-tokens",
};
