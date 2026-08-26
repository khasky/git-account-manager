import { PlatformId } from "./types";

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
