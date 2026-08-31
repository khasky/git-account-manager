import { describe, expect, it } from "vitest";
import { attachedKey } from "./attachedKey";

describe("attachedKey", () => {
  /** The reported case: pick a key in the existing-key list, switch back, hit
   *  Disconnect, and the dialog offered to delete it. The list shows every key
   *  in ~/.ssh, so the offered path belonged to another platform's account. */
  it("offers nothing for a key that was only highlighted in the picker", () => {
    expect(
      attachedKey({
        keyUploaded: false,
        sshPrivateKeyPath: "/home/a/.ssh/id_ed25519_gam_github_octo",
        sshPublicKeyPath: "/home/a/.ssh/id_ed25519_gam_github_octo.pub",
      }),
    ).toEqual({ keyPath: "", pubKeyPath: "" });
  });

  it("offers the pair once the account carries it", () => {
    expect(
      attachedKey({
        keyUploaded: true,
        sshPrivateKeyPath: "/home/a/.ssh/id_ed25519_gam_bitbucket_khasky",
        sshPublicKeyPath: "/home/a/.ssh/id_ed25519_gam_bitbucket_khasky.pub",
      }),
    ).toEqual({
      keyPath: "/home/a/.ssh/id_ed25519_gam_bitbucket_khasky",
      pubKeyPath: "/home/a/.ssh/id_ed25519_gam_bitbucket_khasky.pub",
    });
  });

  it("offers nothing for an account with no key at all", () => {
    expect(
      attachedKey({
        keyUploaded: true,
        sshPrivateKeyPath: "",
        sshPublicKeyPath: "",
      }),
    ).toEqual({ keyPath: "", pubKeyPath: "" });
  });
});
