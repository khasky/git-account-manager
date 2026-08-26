# Security policy

## Reporting a vulnerability

Report privately through GitHub, not in a public issue:

[Open a private security advisory](https://github.com/khasky/git-account-manager/security/advisories/new)

Do not open a public issue, pull request, or discussion for a security problem, and do not post it anywhere else until a fix has shipped.

Useful in a report:

- what an attacker can do, and what they need first (local access, a malicious repository, a hostile network, another app on the machine);
- where it lives — a file, or an app surface (OAuth flow, token storage, SSH key handling, `~/.ssh/config` or `.gitconfig` writes, the pre-push guard, the updater);
- the OS and version, the app version, and how it was installed (MSI, NSIS, `.dmg`, `.AppImage`, `.deb`, or built from source);
- the smallest reproduction you have — a snippet, a screen recording, or numbered steps;
- if it involves a repository or account you cannot share, an equivalent public one.

Never include real credentials, OAuth tokens, private keys, or another person's data. A redacted excerpt is enough.

## What to expect

- Acknowledgement within 7 working days.
- A first assessment (accepted / not a vulnerability / needs more detail) within 14 working days.
- Fixes ship in a normal release; a critical one gets an out-of-band release.
- Public disclosure is coordinated with you once the fix ships, or 90 days after the report — whichever comes first.
- Credit in the release notes and the advisory if you want it — say so in the report.

This is a solo, unpaid project: there is no bug bounty.

## Scope

In scope: this repository — the desktop app and everything it writes or reads on your machine (the OS credential store entries, `profiles.json`, `~/.ssh/config`, global and per-repository Git config, the installed `pre-push` hook), the OAuth flows it drives, and the signed-update path.

Out of scope: vulnerabilities in GitHub, GitLab, Bitbucket, Git itself, OpenSSH, Tauri, or the OS credential store — report those to their own programs. Also out of scope: findings that require physical access to an unlocked device, an attacker who already has code execution as your OS user, or social engineering of the maintainer.

## Safe harbor

Research done in good faith under this policy is welcome. Stay within your own accounts, machines, and repositories, do not degrade anything for anyone else, and give a reasonable window before disclosing. Under those conditions there will be no legal action from this project.

## Supported versions

Only the latest published release receives security fixes. It is listed on the [releases page](https://github.com/khasky/git-account-manager/releases); the [Installation section](README.md#installation) of the README lists every way to get it.
