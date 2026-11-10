import { describe, expect, it } from "vitest";
import { assetName, updaterManifest } from "./release-assets.mjs";

describe("release asset names", () => {
  it.each([
    ["Git.Account.Manager_1.0.0_x64_en-US.msi", "Git-Account-Manager-1.0.0-win-x64.msi"],
    ["Git.Account.Manager_1.0.0_x64-setup.exe", "Git-Account-Manager-1.0.0-win-x64-setup.exe"],
    ["Git.Account.Manager_1.0.0_aarch64.dmg", "Git-Account-Manager-1.0.0-mac-arm64.dmg"],
    ["Git.Account.Manager_1.0.0_x64.dmg", "Git-Account-Manager-1.0.0-mac-x64.dmg"],
    ["Git.Account.Manager_1.0.0_amd64.AppImage", "Git-Account-Manager-1.0.0-linux-x64.AppImage"],
    ["Git.Account.Manager_1.0.0_amd64.deb", "Git-Account-Manager-1.0.0-linux-x64.deb"],
    ["Git-Account-Manager-1.0.0-1.x86_64.rpm", "Git-Account-Manager-1.0.0-linux-x64.rpm"],
    ["Git.Account.Manager_aarch64.app.tar.gz", "Git-Account-Manager-1.0.0-mac-arm64.app.tar.gz"],
    ["Git.Account.Manager_x64.app.tar.gz", "Git-Account-Manager-1.0.0-mac-x64.app.tar.gz"],
  ])("renames %s", (original, expected) => {
    expect(assetName(original, "1.0.0")).toBe(expected);
    expect(assetName(expected, "1.0.0")).toBe(expected);
  });
  it("preserves updater metadata and renames detached signatures", () => {
    expect(assetName("latest.json", "1.0.0")).toBe("latest.json");
    expect(assetName("Git.Account.Manager_1.0.0_x64-setup.exe.sig", "1.0.0")).toBe(
      "Git-Account-Manager-1.0.0-win-x64-setup.exe.sig",
    );
  });
  it.each(["Git.Account.Manager.app.tar.gz", "unknown.zip"])(
    "rejects unsupported or ambiguous asset %s",
    (name) => {
      expect(() => assetName(name, "1.0.0")).toThrow();
    },
  );
});

describe("updater release manifest", () => {
  function release() {
    const payloads = {
      "windows-x86_64": "Git.Account.Manager_1.0.0_x64-setup.exe",
      "linux-x86_64": "Git.Account.Manager_1.0.0_amd64.AppImage",
      "darwin-x86_64": "Git.Account.Manager_x64.app.tar.gz",
      "darwin-aarch64": "Git.Account.Manager_aarch64.app.tar.gz",
    };
    const assets = Object.entries(payloads).flatMap(([platform, name]) => [
      { name },
      { name: `${name}.sig`, signature: `signature for ${platform}` },
    ]);
    return assets;
  }

  it("points every platform at its renamed payload and preserves signatures", () => {
    const assets = release();
    const result = updaterManifest("khasky/git-account-manager", "v1.0.0", assets);
    for (const entry of Object.values(result.platforms)) {
      const name = decodeURIComponent(
        new URL(entry.url).pathname.split("/").pop(),
      );
      expect(assets.map(({ name }) => assetName(name, "1.0.0"))).toContain(
        name,
      );
      const signature = assets.find(
        (asset) => assetName(asset.name, "1.0.0") === `${name}.sig`,
      );
      expect(entry.signature).toBe(signature.signature);
    }
    expect(result.platforms["darwin-aarch64"].url).toContain(
      "Git-Account-Manager-1.0.0-mac-arm64.app.tar.gz",
    );
  });

  it.each(["platform", "payload", "signature", "empty signature"])(
    "rejects an incomplete or incorrect %s before assets change",
    (fault) => {
      const assets = release();
      if (fault === "platform") assets.splice(-2);
      if (fault === "payload") assets.shift();
      if (fault === "signature") assets.pop();
      if (fault === "empty signature") assets[1].signature = " ";
      expect(() =>
        updaterManifest("khasky/git-account-manager", "v1.0.0", assets),
      ).toThrow();
    },
  );

  it("selects the matching Windows installer when MSI and NSIS are signed", () => {
    const assets = release();
    assets.push(
      { name: "Git.Account.Manager_1.0.0_x64_en-US.msi" },
      { name: "Git.Account.Manager_1.0.0_x64_en-US.msi.sig", signature: "msi signature" },
    );
    const { platforms } = updaterManifest("khasky/git-account-manager", "v1.0.0", assets);
    expect(platforms["windows-x86_64-msi"].url).toContain("win-x64.msi");
    expect(platforms["windows-x86_64-msi"].signature).toBe("msi signature");
    expect(platforms["windows-x86_64-nsis"].url).toContain("win-x64-setup.exe");
  });
});
