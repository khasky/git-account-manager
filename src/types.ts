export interface PlatformAccount {
  username: string;
  git_name: string;
  git_email: string;
  ssh_private_key_path: string;
  ssh_public_key_path: string;
  token?: string;
}

export interface Profile {
  id: string;
  name: string;
  default_platform?: string;
  github?: PlatformAccount;
  gitlab?: PlatformAccount;
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
}

export interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}
