# Contributing

Contributions are welcome!

1. Fork the repository.
2. Create a branch: `git checkout -b my-change`.
3. Make your changes and commit them following the convention below.
4. Push the branch and open a pull request.

## Commit messages

This repo follows [Conventional Commits](https://www.conventionalcommits.org/). A `commit-msg` hook (husky + commitlint, installed automatically on `pnpm install`) rejects messages that don't parse, so the format never drifts and the version and changelog are derived straight from history.

Shape — `type(scope)?: subject`:

```
feat(cli): add `gam set` shorthand
fix(updater): handle a missing latest.json on first launch
docs: document the SSH config layout
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `style`, `test`, `build`, `ci`, `chore`, `revert`. `feat` and `fix` drive the version bump (minor / patch) and appear in `CHANGELOG.md`; the rest don't bump or show up there. Scope is optional and free-form (usually the area touched). Mark an incompatible change with a `!` after the type/scope (`feat!: …`) or a `BREAKING CHANGE:` footer.

## Releasing (maintainers)

Run `pnpm release` ([`commit-and-tag-version`](https://github.com/absolute-version/commit-and-tag-version)). It derives the next version from the Conventional Commits since the last `v*` tag, bumps it in **all four** version files in lockstep — `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock` (the last two via the custom updaters in `scripts/`) — updates `CHANGELOG.md`, then creates the release commit and the `v*` tag.

Publish with `git push --follow-tags`. Pushing the tag triggers `.github/workflows/build.yml`, which builds the signed installers for every platform, publishes the GitHub Release, and fills in its notes automatically.

Preview a release without writing anything: `pnpm exec commit-and-tag-version --dry-run`.
