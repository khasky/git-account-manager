export function assetName(name, version) {
  if (name === "latest.json") return name;
  if (name.endsWith(".sig"))
    return `${assetName(name.slice(0, -4), version)}.sig`;
  const formats = [
    [".app.tar.gz", "mac", "app.tar.gz", ""],
    [".msi", "win", "msi", ""],
    ["-setup.exe", "win", "exe", "-setup"],
    [".dmg", "mac", "dmg", ""],
    [".AppImage", "linux", "AppImage", ""],
    [".deb", "linux", "deb", ""],
    [".rpm", "linux", "rpm", ""],
  ];
  const format = formats.find(([ending]) => name.endsWith(ending));
  if (!format) throw new Error(`unclassified release asset: ${name}`);
  const [, os, extension, suffix] = format;
  const architecture = /aarch64|arm64/.test(name)
    ? "arm64"
    : /x64|x86_64|amd64/.test(name)
      ? "x64"
      : null;
  if (!architecture) throw new Error(`no architecture in asset name: ${name}`);
  return `Git-Account-Manager-${version}-${os}-${architecture}${suffix}.${extension}`;
}

export function updaterManifest(repo, tag, assets) {
  const version = tag.replace(/^v/, "");
  const platforms = {};
  const files = new Map(
    assets.map((asset) => [assetName(asset.name, version), asset]),
  );
  const prefix = `https://github.com/${repo}/releases/download/${tag}/`;
  for (const [platform, bundle, suffix, required] of [
    ["windows-x86_64", "nsis", "win-x64-setup.exe", true],
    ["linux-x86_64", "appimage", "linux-x64.AppImage", true],
    ["darwin-x86_64", "app", "mac-x64.app.tar.gz", true],
    ["darwin-aarch64", "app", "mac-arm64.app.tar.gz", true],
    ["windows-x86_64", "msi", "win-x64.msi", false],
    ["linux-x86_64", "deb", "linux-x64.deb", false],
    ["linux-x86_64", "rpm", "linux-x64.rpm", false],
  ]) {
    const name = `Git-Account-Manager-${version}-${suffix}`;
    const payload = files.get(name);
    const signature = files.get(`${name}.sig`)?.signature?.trim();
    if (!payload || !signature) {
      if (required)
        throw new Error(`Missing updater payload or signature: ${name}`);
      continue;
    }
    const entry = {
      signature,
      url: `${prefix}${encodeURIComponent(name)}`,
    };
    platforms[`${platform}-${bundle}`] = entry;
    if (required) platforms[platform] = entry;
  }
  return { version, platforms };
}
