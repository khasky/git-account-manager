# Changelog

All notable changes are documented here. The format follows [Conventional Commits](https://www.conventionalcommits.org/) and the version scheme is [Semantic Versioning](https://semver.org/).

## [1.0.1](https://github.com/khasky/git-account-manager/compare/v1.0.0...v1.0.1) (2026-08-29)

## 1.0.0 (2026-08-29)


### Features

* **git:** read and write git configuration through the git cli ([cad3457](https://github.com/khasky/git-account-manager/commit/cad34572526ed7e4ca4b9808846bac972f5ecc83))
* **guard:** stop an unbound repository from borrowing an identity ([52535bd](https://github.com/khasky/git-account-manager/commit/52535bdf581d4bcc4990d2628f4e19cc2d46051b))
* **i18n:** load the interface strings behind a locale switch ([76ad5a1](https://github.com/khasky/git-account-manager/commit/76ad5a1cc386e95e713a480838a84f611bf3e294))
* **i18n:** translate the interface into ten more locales ([24ac3d1](https://github.com/khasky/git-account-manager/commit/24ac3d1d839e7d3ed3850ad103e74bd8bfdb1986))
* **oauth:** sign a profile in to a platform without pasting a token ([489a462](https://github.com/khasky/git-account-manager/commit/489a4626c73b62fbb08f5991f86e8fcfb086e61c))
* **openssh:** point tortoisegit and the git cli at the managed ssh config ([02f4850](https://github.com/khasky/git-account-manager/commit/02f48503681c35912935c2feb45be6eeeb9aa3c2))
* **platform:** describe each supported host in one table ([4542736](https://github.com/khasky/git-account-manager/commit/4542736c609457e4086bcf321d213a8fc344ef9e))
* **platform:** share one http client across every host call ([7b2a695](https://github.com/khasky/git-account-manager/commit/7b2a6958a550e574a7623990221c23343e70478a))
* **platform:** show a profile's connected accounts and their state ([d8515df](https://github.com/khasky/git-account-manager/commit/d8515dfb3e980c300490b95dc086517001972c1d))
* **platform:** verify accounts and upload keys through each host's api ([8dc475c](https://github.com/khasky/git-account-manager/commit/8dc475c8b63800e5169ef2534955fbbec652e3b3))
* **profiles:** edit a profile, its keys and its repositories in one form ([55ee7d5](https://github.com/khasky/git-account-manager/commit/55ee7d59ff9fe878a055dc8919601425e5d31e73))
* **profiles:** model profiles, platforms and repository bindings ([732f441](https://github.com/khasky/git-account-manager/commit/732f44145e4f169d9414244bd07989d99f1595e8))
* **profiles:** summarize a profile on a card ([8484011](https://github.com/khasky/git-account-manager/commit/8484011b123b44ab64ddfead04075760b147b1e5))
* **repos:** bind an identity to a repository instead of to the machine ([29fbaa0](https://github.com/khasky/git-account-manager/commit/29fbaa0d3b4a60180de4f52aef03648a6fece81b))
* **repos:** explain what binding a repository changes before it changes ([5c581d8](https://github.com/khasky/git-account-manager/commit/5c581d80a626dd3ad222d70ab050f9fb4537ceda))
* **repos:** list a profile's repositories in one panel ([da88a0a](https://github.com/khasky/git-account-manager/commit/da88a0a57d3ba9c86ab868f44a12c94e7cfacfe7))
* **repos:** read a repository's current identity from its config ([d15cc0f](https://github.com/khasky/git-account-manager/commit/d15cc0f3068577d5a8ddaba9816ff4fbd5544520))
* **repos:** show what a scan found and what binding would change ([5883269](https://github.com/khasky/git-account-manager/commit/5883269104fb3fe4f84ed04d626d274c8972d2bf))
* **secrets:** keep platform tokens in the os keychain ([3f71068](https://github.com/khasky/git-account-manager/commit/3f71068e13177f156bce0c560af2f9e55565cf67))
* **settings:** expose startup, theme and language on a settings screen ([0017247](https://github.com/khasky/git-account-manager/commit/0017247dd238dca756d893fb7f724c59d8cc9c3e))
* **ssh:** copy a profile's public key to the clipboard ([963773f](https://github.com/khasky/git-account-manager/commit/963773f5b20d00d108eb570de8bc91f66907e4c2))
* **ssh:** give every profile its own key pair and config block ([0a1993b](https://github.com/khasky/git-account-manager/commit/0a1993b9c0edc206a19eae4fee6980f7498e8e54))
* **storage:** serialize every state change through a single lock ([04173af](https://github.com/khasky/git-account-manager/commit/04173afa2c979fd8e58d1210577133cf8ec8e4b0))
* **tauri:** expose the backend to the webview as commands ([5a128d6](https://github.com/khasky/git-account-manager/commit/5a128d61a7b33b46d977cf1723687f1ca9173b79))
* **tauri:** run child processes without flashing a console window ([a018c15](https://github.com/khasky/git-account-manager/commit/a018c1568e4a15135186f041aa74e9f8c8d50bbb))
* **tauri:** wire the modules, plugins and tray into the app ([71d2595](https://github.com/khasky/git-account-manager/commit/71d25956f48e422ed7defc00ce03bd168600b029))
* **theme:** follow the system theme with a manual override ([c89229d](https://github.com/khasky/git-account-manager/commit/c89229d73d2f323a992f9e4340751cec6c2f6693))
* **tray:** switch the active profile from the system tray ([26d26a8](https://github.com/khasky/git-account-manager/commit/26d26a83a905cfdacb72cd6b8e4c6be9e3d71d27))
* **ui:** add the icon set the screens draw from ([a958b96](https://github.com/khasky/git-account-manager/commit/a958b9693bed89c2b2c2808e3d5f3218add0d231))
* **ui:** add the primitives every screen is built from ([0bfb5fd](https://github.com/khasky/git-account-manager/commit/0bfb5fdda691c48ae0211f39f53441a5b65c9c12))
* **ui:** assemble the screens into the application shell ([47ae7d0](https://github.com/khasky/git-account-manager/commit/47ae7d08232f54fde72ad16b011a5d2c82cdea2d))
* **ui:** call every tauri command through one typed binding ([cb4557f](https://github.com/khasky/git-account-manager/commit/cb4557f5de09bf07040dd8b518d10644ed2bba3f))
* **ui:** define the shape of the state the backend returns ([9288aed](https://github.com/khasky/git-account-manager/commit/9288aedf48b70e30d22b6c33ff494df3b5812d87))
* **updater:** offer the update in-app once one is published ([b8a2a52](https://github.com/khasky/git-account-manager/commit/b8a2a52f807c30a880c0e01e50b7b5ee0c97fa49))
