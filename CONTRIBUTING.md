# Contributing

Contributions are welcome — bug reports, feature requests, and pull requests.

- **To report a bug**, open a [bug report issue](https://github.com/khasky/git-account-manager/issues/new?template=bug_report.md).
- **Anything else** — a [feature request](https://github.com/khasky/git-account-manager/issues/new?template=feature_request.md) or a [question](https://github.com/khasky/git-account-manager/issues/new?template=question.md).
- **Security problems never go in a public issue** — see [SECURITY.md](./SECURITY.md).

Set up your local build first: [Development](README.md#development). Every PR branches off `main` the [way described below](#branches-and-pull-requests), runs the [gates](#pre-pr-gates), and follows the [commit convention](#commit-messages). By contributing you agree your work is licensed under the [MIT License](./LICENSE).

## Where each guide lives

| Doc | What it covers |
| --- | --- |
| [README.md](README.md#development) | Building and running from source: prerequisites, `pnpm dev:desktop`, the bundle, IDE setup |
| [docs/releasing.md](docs/releasing.md) | Maintainer-only: cutting a release, what the tag triggers, recovering from a bad one |
| [SECURITY.md](./SECURITY.md) | Reporting a vulnerability privately |
| [SUPPORT.md](./SUPPORT.md) | Where to ask a question |

## Branches and pull requests

`main` is the only long-lived branch, and it is always releasable: every installer is built from a `v*` tag on `main`, so anything merged there has already passed the [gates below](#pre-pr-gates).

Outside contributors work in a fork; the branches in this repo belong to the maintainer and to Dependabot. The flow is the same either way:

```bash
git switch -c fix/ssh-config-rewrite main
# commits following the convention below
pnpm build:web && cargo test --all-targets --manifest-path src-tauri/Cargo.toml
git push -u origin fix/ssh-config-rewrite
```

- **Name the branch after the commit type it carries** — `feat/…`, `fix/…`, `docs/…`, `refactor/…`, `perf/…`, `ci/…`, `chore/…`.
- **One branch, one change**, opened as a PR while it is still small.
- **Target `main`** — it is the only branch that takes pull requests.

PRs land as a **squash merge**, so `main` keeps a linear history where 1 PR is 1 commit and 1 line in `CHANGELOG.md`. That squashed commit takes its message from the **PR title**, not from the commits inside the branch, so the title itself has to follow [the convention](#commit-messages) (`fix(ssh): keep a hand-written Host block on rewrite`) even when every commit in the branch already does — the release version and changelog are derived from what lands on `main`. The `pr-title` job in `.github/workflows/build.yml` validates it, because the `commit-msg` hook cannot see a PR title.

## Pre-PR gates

A pull request runs the `smoke` job on Linux and Windows. Both platforms are built because a fair amount of the Rust side sits behind `#[cfg(windows)]` or `#[cfg(unix)]`. Reproduce it locally with:

```bash
pnpm check:tauri                                                            # the tauri crate and @tauri-apps/api must share a major/minor
pnpm build:web                                                              # tsc --noEmit plus the Vite build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --all-targets --manifest-path src-tauri/Cargo.toml
```

Formatting and lints gate in CI rather than in a hook: `commit-msg` is the only hook installed, and a fork's clone runs none of ours at all.

Installers are not built for a pull request — bundling needs the signing secrets, which a fork never receives. They are produced when a maintainer pushes a `v*` tag.

## What CI runs, and when

CI is one workflow, [`.github/workflows/build.yml`](.github/workflows/build.yml), with jobs gated on what triggered it. (`mirror.yml`, the other file there, gates nothing: it pushes copies of the repository to the mirror hosts.)

| Job | Runs on | Does |
| --- | --- | --- |
| `pr-title` | pull requests | validates the title a squash merge will use as the commit message |
| `smoke` | pushes and PRs, never tags | the gate above, on Linux and Windows |
| `build` | `v*` tags, `workflow_dispatch` | bundles all four platforms; on a tag, publishes the release |
| `finalize-release` | `v*` tags | renames the installers and writes the release notes |

Your PR reaches `pr-title` and `smoke`. The other two are the maintainer's path — [docs/releasing.md](docs/releasing.md).

## Commit messages

This repo follows [Conventional Commits](https://www.conventionalcommits.org/). A `commit-msg` hook (husky + commitlint, installed automatically on `pnpm install`) rejects messages that don't parse, so the format never drifts and the version and changelog are derived straight from history.

Shape — `type(scope)?: subject`:

```text
feat(repos): bind a repository to a profile from the scan results
fix(updater): handle a missing latest.json on first launch
docs: document the SSH config layout
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `style`, `test`, `build`, `ci`, `chore`, `revert`. The version bump derives from the commits since the last release: a breaking change bumps major, `feat` minor, anything else patch. `feat`, `fix`, `perf`, `refactor`, and `revert` show up in `CHANGELOG.md`; the rest (`docs`, `style`, `test`, `build`, `ci`, `chore`) are hidden from it. The header and the body lines are capped at 100 characters (commitlint enforces both). Footer lines are not: a trailer can carry a URL, a GHSA id or a commit sha, and those are one unbreakable token — so anything too long to wrap goes in a trailer (`Refs: <url>`), not mid-paragraph.

**`chore` and `style` are hidden from the changelog, so nothing a user can see may hide behind them.** `chore` is housekeeping invisible from the outside — dependency bumps, config, release chores. `style` is formatting with no semantic effect: import order, whitespace. A changed UI string, a moved button, a colour, a locale file, a security hardening — those are `fix` or `feat` with the scope of the area, however small the diff. A security fix that never reaches `CHANGELOG.md` is the worst case of this, since the changelog is how someone decides whether to update.

Scope is the area touched, and it is optional. `commitlint` carries the list of scopes already in use and **warns** on anything else: the warning is there to catch a typo or a second name for an area that has one (`repo` for `repos`, `keys` for `ssh`), not to block a new area — a scope that's genuinely new lands with the warning, and the list grows in `commitlint.config.js`.

Two footers are enforced rather than suggested:

- **`revert` needs `Refs: <sha>`** naming the commit it undoes. Prose in the body is for the why; the trailer is what pairs the two commits.
- **A `!` header needs a `BREAKING CHANGE: <what breaks>` footer.** The `!` says something breaks and can't say what; the footer is the line that lands under **Breaking Changes** in `CHANGELOG.md`.

```text
revert(guard): drop the pre-push hook auto-install on bind

It overwrote a hooksPath-managed hook on two setups before the ownership
check landed.

Refs: 79a8186
```

Keep one concern per commit and per PR: a title that needs a comma-separated list of changes is two PRs.

## Releasing

Maintainer-only, and it needs the signing secrets — a fork cannot cut one. The runbook lives in [docs/releasing.md](docs/releasing.md): what to run, what the tag triggers, and how to recover from a release that came out wrong.

What matters on the contributor side is that the version and the changelog are derived from what lands on `main`, so the [commit convention](#commit-messages) above is what decides them.
