import type { PlatformId } from "./types";

/** The platforms this app knows, in the order every list and menu shows them. */
export const PLATFORMS: readonly PlatformId[] = [
  "github",
  "gitlab",
  "bitbucket",
  "azure",
  "codeberg",
  "huggingface",
];

export const PLATFORM_LABEL: Record<PlatformId, string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket: "Bitbucket",
  azure: "Azure DevOps",
  codeberg: "Codeberg",
  huggingface: "Hugging Face",
};

const PROFILE_URL_BASE: Record<PlatformId, string> = {
  github: "https://github.com/",
  gitlab: "https://gitlab.com/",
  bitbucket: "https://bitbucket.org/",
  // An Azure DevOps account here is an organization, so this is its home.
  azure: "https://dev.azure.com/",
  codeberg: "https://codeberg.org/",
  huggingface: "https://huggingface.co/",
};

/** Where a platform that will not take a key from a token has it pasted. */
export const HUGGINGFACE_KEYS_URL = "https://huggingface.co/settings/keys";

export function profileUrl(platform: PlatformId, username: string): string {
  return PROFILE_URL_BASE[platform] + username;
}

/** The organization out of what a user typed or pasted: its bare name, a
 *  `dev.azure.com/<org>` URL or a legacy `<org>.visualstudio.com` host. Mirrors
 *  `platform::azure_org_from_input`, which stores the same answer on connect.
 *  Empty when nothing usable is there. */
export function azureOrganization(input: string): string {
  let raw = input.trim().replace(/^https?:\/\//i, "");
  const slash = raw.indexOf("/");
  const at = (slash < 0 ? raw : raw.slice(0, slash)).lastIndexOf("@");
  if (at >= 0) raw = raw.slice(at + 1);
  const host = raw.split("/")[0].toLowerCase();

  let org: string;
  if (raw.toLowerCase().startsWith("dev.azure.com/")) {
    org = raw.slice("dev.azure.com/".length).split("/")[0];
  } else if (host.endsWith(".visualstudio.com")) {
    org = raw.split("/")[0].slice(0, -".visualstudio.com".length);
  } else {
    org = raw.replace(/\/+$/, "");
  }
  return /^[A-Za-z0-9._-]+$/.test(org) ? org : "";
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
      return azureSettingsUrl(username, "tokens");
    case "codeberg":
      return "https://codeberg.org/user/settings/applications";
    case "huggingface":
      return "https://huggingface.co/settings/tokens";
  }
}

/** Azure DevOps keeps SSH keys per organization, and a token cannot add one,
 *  so this is where the user pastes it. */
export function azureKeysPageUrl(organization: string): string {
  return azureSettingsUrl(organization, "keys");
}

function azureSettingsUrl(input: string, page: "tokens" | "keys"): string {
  const org = azureOrganization(input);
  return org
    ? `https://dev.azure.com/${encodeURIComponent(org)}/_usersSettings/${page}`
    : "https://dev.azure.com/";
}
