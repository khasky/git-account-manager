import type { PlatformState } from "./components/PlatformSection";

/** The parts of a platform section this rule reads. */
type KeyState = Pick<
  PlatformState,
  "keyUploaded" | "sshPrivateKeyPath" | "sshPublicKeyPath"
>;

export interface AttachedKey {
  keyPath: string;
  pubKeyPath: string;
}

/**
 * The key pair an account actually carries, as opposed to one merely
 * highlighted in the picker.
 *
 * Browsing the existing-key list writes the highlighted paths into the section
 * immediately, before anything is uploaded and while the list shows every key
 * in `~/.ssh` — including the ones other profiles depend on. Treating that
 * selection as the account's key put a working key of another platform one
 * click from deletion on disconnect. `keyUploaded` is what separates the two:
 * it is set by generating, by uploading, and by loading a saved profile, and
 * by nothing else.
 */
export function attachedKey(state: KeyState): AttachedKey {
  if (!state.keyUploaded) {
    return { keyPath: "", pubKeyPath: "" };
  }
  return {
    keyPath: state.sshPrivateKeyPath,
    pubKeyPath: state.sshPublicKeyPath,
  };
}
