<!-- Thanks for contributing to Git Account Manager! -->

## What does this PR do

<!-- A short summary of the change and why it's needed. Link any related issue
     with "Closes #123". -->

## Type of change

- [ ] Bug fix
- [ ] Feature / improvement
- [ ] Docs / chore

## Checklist

<!-- The PR title is what reaches `main` on a squash merge, so it must follow
     the commit convention — see CONTRIBUTING.md. -->

- [ ] PR title follows Conventional Commits (`fix(ssh): …`)
- [ ] `pnpm build:web` passes (typecheck + frontend build)
- [ ] `cargo test --all-targets --manifest-path src-tauri/Cargo.toml` passes
- [ ] Rust changes behind `#[cfg(windows)]` / `#[cfg(unix)]` were checked on both, or the platform is named above
- [ ] User-visible strings added to every `src/i18n/*.ts` locale, if the PR adds any
- [ ] No secret, token, or private key path committed
