import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { assetName, updaterManifest } from "./lib/release-assets.mjs";
import { buildReleaseNotes } from "./lib/release-notes.mjs";

const { REPO: repo, TAG: tag } = process.env;
if (!repo || !tag) throw new Error("REPO and TAG are required");
const gh = (...args) => execFileSync("gh", args, { encoding: "utf8" });
const release = JSON.parse(
  gh("release", "view", tag, "--repo", repo, "--json", "databaseId,isDraft"),
);
if (!release.isDraft) throw new Error("Only draft releases can be finalized");
const assets = JSON.parse(
  gh(
    "api",
    `repos/${repo}/releases/${release.databaseId}/assets`,
    "--paginate",
    "--slurp",
  ),
).flat();
if (assets.some((asset) => asset.state !== "uploaded" || asset.size <= 0)) {
  throw new Error("Release has empty or incomplete assets");
}
for (const asset of assets) {
  if (asset.name.endsWith(".sig")) {
    asset.signature = gh(
      "api",
      `repos/${repo}/releases/assets/${asset.id}`,
      "-H",
      "Accept: application/octet-stream",
    );
  }
}
const manifest = updaterManifest(repo, tag, assets);
manifest.pub_date = new Date().toISOString();
manifest.notes = buildReleaseNotes(
  readFileSync("CHANGELOG.md", "utf8"),
  tag.replace(/^v/, ""),
);
const renames = assets.map((asset) => ({
  ...asset,
  renamed: assetName(asset.name, tag.replace(/^v/, "")),
}));
for (const asset of renames) {
  const name = asset.renamed;
  if (name === asset.name) continue;
  gh(
    "api",
    "--method",
    "PATCH",
    `repos/${repo}/releases/assets/${asset.id}`,
    "-f",
    `name=${name}`,
  );
  console.log(`${asset.name} -> ${name}`);
}
const directory = "target/release-assets";
mkdirSync(directory, { recursive: true });
const filename = `${directory}/latest.json`;
writeFileSync(filename, `${JSON.stringify(manifest, null, 2)}\n`);
gh("release", "upload", tag, filename, "--repo", repo, "--clobber");
