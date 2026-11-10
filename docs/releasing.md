# Releasing (maintainers)

The runbook for cutting a release: what to run, what the tag triggers, and what to do when a release comes out wrong. Contributors want [CONTRIBUTING.md](../CONTRIBUTING.md) instead — nothing here is needed to open a pull request.

## Before any release

`main` is the only branch releases are cut from, and every installer comes from a `v*` tag on it. Two things have to hold before you tag:

```bash
git status --short   # clean, except what you are about to release
git log --oneline    # the commits since the last tag say what the version will be
```

Rehearse the packaging when the `[package.metadata.packager]` config, the icons, or a platform dependency changed — `smoke` compiles the Rust side but never packages, so that path is unverified until a build runs:

```bash
gh workflow run "Build & Release" --ref main
```

A `workflow_dispatch` run builds all four platforms and signs them and keeps them as CI artifacts without publishing anything. Failing there costs a rerun; failing on a tag leaves a half-published release to clean up.

## The first release

The very first tag takes `--first-release`: it skips the bump and tags the version already in `package.json`, while still writing the changelog and creating the release commit:

```bash
pnpm release --first-release --dry-run
pnpm release --first-release
git push --follow-tags origin main
```

Because no earlier tag exists, this first `CHANGELOG.md` section spans the entire commit history: every `feat`, `fix`, `perf`, `refactor`, and `revert` commit appears, while `docs`, `style`, `test`, `build`, `ci`, and `chore` stay hidden. The `--dry-run` above prints that section without writing anything; to trim it, edit `CHANGELOG.md` and `git commit --amend` before pushing. A `CHANGELOG.md` committed by hand before this point is not a problem either — the `prechangelog` hook (`scripts/clear-changelog-seed.mjs`) empties a seed-only file so its header cannot end up duplicated.

The flag must not be reused — a second run would tag a version that already exists.

## A normal release

```bash
pnpm release --dry-run   # prints the computed version and changelog section, writes nothing
pnpm release
git push --follow-tags origin main
```

`pnpm release` ([`commit-and-tag-version`](https://github.com/absolute-version/commit-and-tag-version)) derives the next version from the Conventional Commits since the last `v*` tag — a breaking change bumps major, `feat` minor, anything else patch — and bumps it in **all three** version files in lockstep: `package.json`, `Cargo.toml`, and `Cargo.lock` (the last two through the custom updaters in `scripts/`). It then writes the `CHANGELOG.md` section, creates the `chore(release): <version>` commit, and tags it.

Override the derived version when it is wrong — a release that is really a major, a version pinned for other reasons:

```bash
pnpm release --release-as minor
pnpm release --release-as 1.0.0
```

## What the tag triggers

Pushing the tag starts [`.github/workflows/build.yml`](../.github/workflows/build.yml). Only a `v*` tag (or a manual dispatch) reaches the `build` job — a push to `main` runs `smoke` alone, because packaging four platforms per commit spends runner-minutes on artifacts nobody downloads.

| Job | Runs on | Does |
| --- | --- | --- |
| `pr-title` | pull requests | validates the title a squash merge will use as the commit message |
| `smoke` | pushes, PRs and tags | Biome and Vitest for `scripts/`, `cargo fmt`/`clippy`/`test` on Linux and Windows |
| `create-release` | `v*` tags, after `smoke` | creates the release as a draft |
| `build` | `v*` tags, `workflow_dispatch` | packages Windows, Linux, macOS ARM and macOS Intel with `cargo-packager`; on a tag, signs and uploads them to the draft |
| `finalize-release` | `v*` tags, after all four builds | renames installers, writes `latest.json`, drops `.sig` assets, writes the release notes |

`finalize-release` needs every platform to finish, so a single failed build leaves a draft under `cargo-packager`'s own filenames with a placeholder body and no `latest.json` — that state is the symptom, not a separate bug. The draft is never published automatically: check the assets, then publish it on the Releases page. Until then `releases/latest/download/latest.json` still points at the previous release, so installed copies see nothing.

## What ends up on the release

Nine assets, plus the two source archives GitHub attaches on its own:

```text
Git-Account-Manager-<version>-win-x64-setup.exe
Git-Account-Manager-<version>-win-x64.msi
Git-Account-Manager-<version>-mac-arm64.dmg
Git-Account-Manager-<version>-mac-x64.dmg
Git-Account-Manager-<version>-linux-x64.AppImage
Git-Account-Manager-<version>-linux-x64.deb
Git-Account-Manager-<version>-mac-arm64.app.tar.gz
Git-Account-Manager-<version>-mac-x64.app.tar.gz
latest.json
```

`cargo-packager` names its output after the product name, so `scripts/rename-release-assets.mjs` renames the installers once every platform has uploaded, and writes `latest.json` from the renamed assets in the same step, with each detached signature embedded — an updater aimed at a URL that no longer resolves is worse than an inconsistent filename. The script refuses to touch anything unless every required payload (NSIS setup, AppImage, both `.app.tar.gz`) has a signature, and it fails the job on any asset it cannot classify rather than leaving one file under the old scheme. The rules are covered by `scripts/lib/release-assets.test.mjs`:

```bash
pnpm test
```

RPM is no longer built: `cargo-packager` has no RPM format. Use the AppImage or the `.deb` instead.

The two `.app.tar.gz` archives, packed by the `build` job from the `.app`, are not clutter: they are the only format the macOS updater can install, since it replaces the `.app` bundle in place and cannot mount a DMG. Renaming them is safe because the updater reads their URL out of `latest.json`, which the same step rewrites, and the signature covers the bytes rather than the asset name. Windows and Linux update straight from the `-setup.exe` and the AppImage, which is why they need no extra payload.

The detached `.sig` files are deleted in the step after the rename. Every signature the updater verifies is embedded in `latest.json`, which is what it reads; the loose files were downloaded by nobody and only padded the list.

## Release notes

`finalize-release` slices the version's section out of `CHANGELOG.md` and writes it into the body:

```bash
node scripts/release-notes.mjs "${TAG#v}" release-notes.md
gh release edit "$TAG" --repo "$REPO" --notes-file release-notes.md
```

It does **not** use GitHub's `generate-notes`, which builds its "What's Changed" list from the pull requests in the tag range. Every commit here lands on `main` as a direct push, so that list is always empty and the endpoint returns nothing but a compare link over an otherwise blank body. `CHANGELOG.md` is written by `commit-and-tag-version` from the commits themselves and needs no pull requests to say anything.

The extraction is covered by `scripts/lib/release-notes.test.mjs`: that a shorter version cannot claim a longer one's section (`0.1.3` vs `0.1.30`), that the compare link is lifted out of the heading and re-appended as a footer, and that a version carrying only hidden commit types says so instead of pointing at an equally silent file.

Rewrite the body of a release whose job failed after the build:

```bash
node scripts/release-notes.mjs <version> release-notes.md
gh release edit v<version> --notes-file release-notes.md
```

## Signing and the updater

Installers are signed with a [minisign](https://jedisct1.github.io/minisign/) key pair; the Tauri CLI (`tauri signer sign`) writes each `.sig`, because the minisign inside `cargo packager signer` cannot decrypt a key whose password is empty. It is the same key pair the Tauri releases were signed with, so copies installed before the move to GPUI keep accepting updates. The private half lives in the `TAURI_SIGNING_PRIVATE_KEY` repository secret, with `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` alongside it when the key carries a passphrase; the secret names stayed so nothing had to be re-entered. The public half is `PUBLIC_KEY` in `src/updater.rs`.

A local `pnpm build:desktop` signs nothing. The `build` job checks the secret before building and fails if it is missing, since unsigned payloads would leave `finalize-release` unable to write `latest.json`.

**Losing the private key ends the update path for every installed copy.** A new key means a new `PUBLIC_KEY` in `src/updater.rs`, and only builds signed by the old key are accepted by the apps already out there; those users have to reinstall by hand.

## When a release goes wrong

The tag is the input to everything, so recovery depends on whether the tag itself is wrong.

**The build failed, the tag is right.** Fix the cause on `main`, then move the tag onto the fix and force-push it. Re-running the old workflow would rebuild the same broken commit:

```bash
git tag -f v<version> <sha>
git push --force origin v<version>
```

Delete the draft first (**Releases → the release → delete**), otherwise `create-release` reuses it and the old assets survive alongside the new ones.

**The version itself is wrong.** Nothing published yet — drop the tag and the release commit, then run `pnpm release` again:

```bash
git tag -d v<version>
git reset --hard HEAD~1
```

**The release is already public and installed.** Do not move the tag. Cut a new patch release: a moved tag changes what a URL in `latest.json` points at, and an updater that downloads different bytes than the signature covers refuses the update.
