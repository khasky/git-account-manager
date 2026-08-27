# Changelog

All notable changes to Git Account Manager are documented here. The format follows [Conventional Commits](https://www.conventionalcommits.org/) and the version scheme is [Semantic Versioning](https://semver.org/).

Entries are generated automatically by `pnpm release` (`commit-and-tag-version`).

## [0.2.0](https://github.com/khasky/git-account-manager/compare/v0.1.0...v0.2.0) (2026-08-27)


### BREAKING CHANGES

* a profile whose chosen platform has been disconnected
now falls back to its first connected account instead of reporting no
identity at all, so the machine-wide identity is written where it
previously was skipped.

### Features

* harden the backend and split the oversized components ([01d55a7](https://github.com/khasky/git-account-manager/commit/01d55a7dd1e13492c7bfae287eb4209bbe2af7d6))
* **repos:** configure folders inside the profile that owns them ([575f682](https://github.com/khasky/git-account-manager/commit/575f68253958f38accdf3fbee555983f3567b4f9))
* **repos:** make both repository switches undo themselves ([7426a56](https://github.com/khasky/git-account-manager/commit/7426a56a847aad6f49420420eb984e116e205245))
* **ui:** show what is running and what each repository option means ([876af71](https://github.com/khasky/git-account-manager/commit/876af71d9cb23870d3c52747791dd830e314f656))


### Bug Fixes

* **ci:** repair the release notes step and drop .sig assets ([e214f72](https://github.com/khasky/git-account-manager/commit/e214f7295225fc7d79c27f4542a56dc535bc6926))
* **openssh:** let the Windows-only integration compile clean elsewhere ([9720b8e](https://github.com/khasky/git-account-manager/commit/9720b8e55d3d82e3ffd22c09913f97a0c63c77d2))
* **repos:** install the guard into husky's hook slot ([5782c5a](https://github.com/khasky/git-account-manager/commit/5782c5ab7a3bc21e5de31b5672c5a42aa4d4b521))
* **repos:** prove repository access with the profile's key, not a token ([b8c1b1f](https://github.com/khasky/git-account-manager/commit/b8c1b1f638bec2b75fbd97e04d8ac8fb54bb4efc))
* **repos:** remove the guard from where it was installed ([acec334](https://github.com/khasky/git-account-manager/commit/acec334a84c0a505b8553395ae0f3393fb7dbcaa))
* **ui:** keep an info tip inside the window ([5e08c8e](https://github.com/khasky/git-account-manager/commit/5e08c8ee1011a344b3cf8f682fdcd74e64c4ae02))


### Performance

* **oauth:** send the token requests through the shared client ([2a6a5e4](https://github.com/khasky/git-account-manager/commit/2a6a5e4ab2fe88737f4a5e11e6dd34ba5a047535))
* **repos:** inspect every bound repository at once ([ad7926b](https://github.com/khasky/git-account-manager/commit/ad7926b89d069dabab6e6f6655b58ce5eb20bd82))
* **repos:** run the repository commands off the main thread ([02b55d6](https://github.com/khasky/git-account-manager/commit/02b55d6f58a018d4b7b09dafb8b531494c43649c))
* **ui:** render the profile list before the health report arrives ([1092620](https://github.com/khasky/git-account-manager/commit/109262067f013db6d2c81f6ea8ad2e5e1c840560))
* **ui:** show a profile's folders without waiting for the doctor ([20aff91](https://github.com/khasky/git-account-manager/commit/20aff911e400a088b3fc4f9cf551085bbf3da235))

## 0.1.0 (2026-08-26)


### Features

* initial public release ([374aa62](https://github.com/khasky/git-account-manager/commit/374aa622973725d1e05445fe69acec2657531209))


### Bug Fixes

* **release:** keep the generator note in the changelog header ([4d35de1](https://github.com/khasky/git-account-manager/commit/4d35de1b5abb812b41cf4249f8184e0e1817f363))
