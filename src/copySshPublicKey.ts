import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/** Reads the public key file and copies its contents to the system clipboard. */
export async function copySshPublicKey(publicKeyPath: string): Promise<void> {
  const content = await invoke<string>("read_public_key", {
    path: publicKeyPath,
  });
  await writeText(content.trim());
}
