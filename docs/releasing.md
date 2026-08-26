# Releasing (maintainers)

The runbook for cutting a release: what to run, what the tag triggers, and what to do when a release comes out wrong. Contributors want [CONTRIBUTING.md](../CONTRIBUTING.md) instead — nothing here is needed to open a pull request.

## Before any release

`main` is the only branch releases are cut from, and every installer comes from a `v*` tag on it. Two things have to hold before you tag:

```bash
git status --short   # clean, except what you are about to release
git log --oneline    # the commits since the last tag say what the version will be
```

Rehearse the bundle when the bundler config, the icons, or a platform dependency changed — `smoke` compiles the Rust side but never bundles, so that path is unverified until a build runs:

```bash
gh workflow run "Build & Release" --ref main
```

A `workflow_dispatch` run builds all four platforms with the release config and keeps the installers as CI artifacts without publishing anything. Failing there costs a rerun; failing on a tag leaves a half-published release to clean up.

## A normal release

```bash
pnpm release --dry-run   # prints the computed version and changelog section, writes nothing
pnpm release
git push --follow-tags origin main
```

`pnpm release` ([`commit-and-tag-version`](https://github.com/absolute-version/commit-and-tag-version)) derives the next version from the Conventional Commits since the last `v*` tag — a breaking change bumps major, `feat` minor, anything else patch — and bumps it in **all four** version files in lockstep: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock` (the last two through the custom updaters in `scripts/`). It then writes the `CHANGELOG.md` section, creates the `chore(release): <version>` commit, and tags it.

Override the derived version when it is wrong — a release that is really a major, a version pinned for other reasons:

```bash
pnpm release --release-as minor
pnpm release --release-as 1.0.0
```

`--first-release` was a one-time flag for `v0.1.0` and must not be reused: it skips the bump, so a second run would tag a version that already exists.

## What the tag triggers

Pushing the tag starts [`.github/workflows/build.yml`](../.github/workflows/build.yml). Only a `v*` tag (or a manual dispatch) reaches the `build` job — a push to `main` runs `smoke` alone, because bundling four platforms per commit spent runner-minutes on artifacts nobody downloaded.

| Job | Runs on | Does |
| --- | --- | --- |
| `pr-title` | pull requests | validates the title a squash merge will use as the commit message |
| `smoke` | pushes and PRs, never tags | typecheck, `cargo fmt`/`clippy`/`test` on Linux and Windows |
| `build` | `v*` tags, `workflow_dispatch` | bundles Windows, Linux, macOS ARM and macOS Intel; on a tag, publishes the release |
| `finalize-release` | `v*` tags, after all four builds | renames installers, drops `.sig` assets, writes the release notes |

Budget roughly 6 minutes end to end; Linux is the long pole. `finalize-release` needs every platform to finish, so a single failed build leaves the release under Tauri's own filenames with a placeholder body — that state is the symptom, not a separate bug.

## What ends up on the release

Ten assets, plus the two source archives GitHub attaches on its own:

```text
Git-Account-Manager-<version>-win-x64-setup.exe
Git-Account-Manager-<version>-win-x64.msi
Git-Account-Manager-<version>-mac-arm64.dmg
Git-Account-Manager-<version>-mac-x64.dmg
Git-Account-Manager-<version>-linux-x64.AppImage
Git-Account-Manager-<version>-linux-x64.deb
Git-Account-Manager-<version>-linux-x64.rpm
Git.Account.Manager_aarch64.app.tar.gz
Git.Account.Manager_x64.app.tar.gz
latest.json
```

Tauri has no artifact-name template, so `scripts/rename-release-assets.sh` renames the installers after tauri-action publishes them, and re-points `latest.json` at the new URLs in the same step — an updater aimed at a URL that no longer resolves is worse than an inconsistent filename. The script fails the job on any asset it cannot classify rather than leaving one file under the old scheme. Check the rules without a release:

```bash
bash scripts/rename-release-assets.sh --self-test
```

The two `.app.tar.gz` archives keep Tauri's names and are not clutter: they are the only format the macOS updater can install, since it replaces the `.app` bundle in place and cannot mount a DMG. Windows and Linux update straight from the `.msi`/`-setup.exe` and the AppImage, which is why they need no extra payload.

The detached `.sig` files are deleted in the step after the rename. Every signature the updater verifies is embedded in `latest.json`, which is what it reads; the loose files were downloaded by nobody and only padded the list.

## Release notes

`finalize-release` generates the notes through the dedicated endpoint and writes them into the body:

```bash
notes=$(gh api --method POST "repos/$REPO/releases/generate-notes" -f tag_name="$TAG" -q .body)
gh api --method PATCH "repos/$REPO/releases/$id" -f body="$notes"
```

Two calls rather than one for a reason worth remembering: `generate_release_notes` is accepted only by *Create a release*. Passing it to *Update a release* is silently dropped and still answers `200`, so the step goes green while tauri-action's placeholder body survives — the failure mode is a release that looks fine in CI and wrong on the page.

The body is the changelog GitHub derives from the commit range. `CHANGELOG.md` in the repo is the version written by `pnpm release`; the two are generated independently and are not expected to match word for word.

## Signing and the updater

Installers and `latest.json` are signed with a [minisign](https://jedisct1.github.io/minisign/) key pair created by `pnpm tauri signer generate`. The private half lives in the `TAURI_SIGNING_PRIVATE_KEY` repository secret, with `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` alongside it when the key carries a passphrase. The public half sits in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

The base config keeps `createUpdaterArtifacts: false` so a local `pnpm build:desktop` works without the key; CI merges `src-tauri/tauri.updater.conf.json` to turn it on. A build without the secret produces installers but no usable `latest.json`, and every installed copy silently stops seeing updates — the app reports nothing, so this fails quietly.

**Losing the private key ends the update path for every installed copy.** A new key means a new `pubkey` in the config, and only builds signed by the old key are accepted by the apps already out there; those users have to reinstall by hand.

## When a release goes wrong

The tag is the input to everything, so recovery depends on whether the tag itself is wrong.

**The build failed, the tag is right.** Fix the cause on `main`, then move the tag onto the fix and force-push it. Re-running the old workflow would rebuild the same broken commit:

```bash
git tag -f v<version> <sha>
git push --force origin v<version>
```

Delete the partly-published release first (**Releases → the release → delete**), otherwise tauri-action appends to it and the old assets survive alongside the new ones.

**The version itself is wrong.** Nothing published yet — drop the tag and the release commit, then run `pnpm release` again:

```bash
git tag -d v<version>
git reset --hard HEAD~1
```

**The release is already public and installed.** Do not move the tag. Cut a new patch release: a moved tag changes what a URL in `latest.json` points at, and an updater that downloads different bytes than the signature covers refuses the update.
