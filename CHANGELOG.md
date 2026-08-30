# Changelog

All notable changes are documented here. The format follows [Conventional Commits](https://www.conventionalcommits.org/) and the version scheme is [Semantic Versioning](https://semver.org/).

## 1.0.0 (2026-08-30)


### Features

* **git:** model the domain the backend and the webview share ([0a06ddd](https://github.com/khasky/git-account-manager/commit/0a06ddde00c8716afd9f6c6f4bf51b24d9a106ac))
* **git:** read and write the machine-wide identity ([ea7e07e](https://github.com/khasky/git-account-manager/commit/ea7e07eb0238859684c9f12cd90da1558ad28836))
* **guard:** refuse a commit from a repository nobody claimed ([d6c83cf](https://github.com/khasky/git-account-manager/commit/d6c83cfa5d0dee8609dea1923b3cc0c255892838))
* **i18n:** switch the language from a provider ([695f179](https://github.com/khasky/git-account-manager/commit/695f179c1a22069eff1a7a8bfed8f086e636a26e))
* **i18n:** translate the interface into ten more locales ([d51ed2b](https://github.com/khasky/git-account-manager/commit/d51ed2b754bed836f238afd966397986fd64b13f))
* **i18n:** write every interface string once in english ([6ca4e8a](https://github.com/khasky/git-account-manager/commit/6ca4e8a5084ad5c027093b7b7e36966f7f12395d))
* **oauth:** connect an account without a pasted token ([8867ab6](https://github.com/khasky/git-account-manager/commit/8867ab69afedfc1d0e8f561baad0c6df1763549b))
* **openssh:** point the windows git tools at openssh ([981e5aa](https://github.com/khasky/git-account-manager/commit/981e5aaf079b91537375a618e17ddea2fedc48c0))
* **platform:** name the hosts and where a profile lives on each ([83b3843](https://github.com/khasky/git-account-manager/commit/83b3843772de059c74f159aaa9cba1d3dc936ad6))
* **platform:** reach each host over one pooled client ([4720174](https://github.com/khasky/git-account-manager/commit/4720174e1ff885ea18c407b8fbdd1d75aafe98b3))
* **profiles:** connect an account on each platform ([203619e](https://github.com/khasky/git-account-manager/commit/203619e2a8ccb6f63d15dad771a5ab8e3865f31f))
* **profiles:** edit a profile, its accounts and its folders ([8901d17](https://github.com/khasky/git-account-manager/commit/8901d17f6c8e58ae42f9f570ec39acf31ec1a304))
* **profiles:** show a profile and what it is connected to ([b6aed75](https://github.com/khasky/git-account-manager/commit/b6aed75959ed3b0fe08ea110bc9bdb242083dd98))
* **repos:** bind an identity to a repository instead of the machine ([7d7cb4a](https://github.com/khasky/git-account-manager/commit/7d7cb4ab9f423569049b8784220f84ce515a46ee))
* **repos:** decide which repositories bind without asking ([c9488c5](https://github.com/khasky/git-account-manager/commit/c9488c584f970d5ea238e5db18c46f8998474dce))
* **repos:** list a folder's repositories with what the scan saw ([e046e34](https://github.com/khasky/git-account-manager/commit/e046e34a0d610749fe8fa563dd044e63873b511d))
* **repos:** pick the folders a profile watches and scan them ([cc379b7](https://github.com/khasky/git-account-manager/commit/cc379b779d1a1cdda4b47d6cc81600acc248e7a9))
* **repos:** report the repositories that drifted ([fb4fafb](https://github.com/khasky/git-account-manager/commit/fb4fafb10ae1b03cc92b12aff3b85b3bab1cdbd8))
* **repos:** turn a folder draft into the writes the backend performs ([43848e2](https://github.com/khasky/git-account-manager/commit/43848e2e90aaa7eb7ef8438db95b753bdc36d132))
* **secrets:** keep every token in the os credential store ([082a7d6](https://github.com/khasky/git-account-manager/commit/082a7d641a803e00215261b8b997b2e72d007b02))
* **settings:** expose startup, theme, language and the guard rails ([15a1816](https://github.com/khasky/git-account-manager/commit/15a181659bff4262cb72d44cfa41732f23e137c5))
* **ssh:** generate a key per account and route each host to it ([c785037](https://github.com/khasky/git-account-manager/commit/c78503724506964157b57ddf1f68bf8bc3941b8a))
* **storage:** serialize every state change through one lock ([b828bbc](https://github.com/khasky/git-account-manager/commit/b828bbcd3068b7125e80643ebd4601c320d8e39c))
* **tauri:** expose the backend as commands and start the app ([a208d17](https://github.com/khasky/git-account-manager/commit/a208d17a8c5a4f0ea162fd1cdc7f115f6e606f07)), closes [#15](https://github.com/khasky/git-account-manager/issues/15) [#17](https://github.com/khasky/git-account-manager/issues/17) [#18](https://github.com/khasky/git-account-manager/issues/18) [#19](https://github.com/khasky/git-account-manager/issues/19) [#20](https://github.com/khasky/git-account-manager/issues/20)
* **theme:** follow the system theme and remember an override ([5aa6532](https://github.com/khasky/git-account-manager/commit/5aa653283a9225b69b48dd33a26de0ee8c549fcc))
* **tray:** switch a profile without opening the window ([b1aae27](https://github.com/khasky/git-account-manager/commit/b1aae2766e874de61a18f53ec7ab33d49a4e2ee4))
* **ui:** add the primitives every screen is built from ([1fd5044](https://github.com/khasky/git-account-manager/commit/1fd5044958963ba85c302941463793f3ee26ad24))
* **ui:** assemble the screens into the application shell ([c189da5](https://github.com/khasky/git-account-manager/commit/c189da525a9d54f7ed9d6cd996565421973e3837))
* **ui:** draw every icon from one file ([325014d](https://github.com/khasky/git-account-manager/commit/325014d17ace5bc1bbe24c2cc2a401463a2177aa))
* **ui:** name every backend command in one place ([2b5bea6](https://github.com/khasky/git-account-manager/commit/2b5bea62273a68c86879f6ff0cf85269106ab48b))
* **ui:** type the state the backend hands the interface ([09e3578](https://github.com/khasky/git-account-manager/commit/09e3578a80fd99cfb68269ea71d1fa46b3ccc92a))
* **updater:** install a newer signed build in one click ([339579a](https://github.com/khasky/git-account-manager/commit/339579a593ce440da4cae35188ea2ceabddd6475))
