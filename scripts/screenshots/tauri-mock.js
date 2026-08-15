/**
 * A fake Tauri backend for the screenshot run.
 *
 * Injected into the page before any app script, so `invoke()` — which is only
 * ever `window.__TAURI_INTERNALS__.invoke` — answers from the fixture below
 * instead of an IPC bridge that does not exist in a browser. Nothing in `src/`
 * changes, and nothing here touches a real account, key, or repository: every
 * name, address and path is invented, and every address uses a `.example`
 * domain or a `users.noreply.github.com` one.
 *
 * Loaded with `addInitScript`, so it is a plain script, not a module.
 */
(() => {
  const HOME = "C:/Users/nadia";
  const P_WORK = "8f2b1c40-3d5e-4a91-b8c2-71ea9d4f6a30";
  const P_OSS = "1c7d94a2-5b60-4e83-9f11-2ad6c803be74";
  const P_NW = "b53e08f7-9c42-4d16-a70b-5e81f2947cd3";

  const key = (name) => ({
    name,
    private_key_path: `${HOME}/.ssh/${name}`,
    public_key_path: `${HOME}/.ssh/${name}.pub`,
  });

  const account = (username, gitName, gitEmail, keyName) => ({
    username,
    git_name: gitName,
    git_email: gitEmail,
    ssh_private_key_path: `${HOME}/.ssh/${keyName}`,
    ssh_public_key_path: `${HOME}/.ssh/${keyName}.pub`,
  });

  const PROFILES = [
    {
      id: P_WORK,
      name: "Work",
      default_platform: "github",
      is_active: true,
      github: account(
        "nadia-acme",
        "Nadia Petrenko",
        "48213574+nadia-acme@users.noreply.github.com",
        "id_ed25519_github_work",
      ),
      gitlab: account(
        "n.petrenko",
        "Nadia Petrenko",
        "n.petrenko@acme.example",
        "id_ed25519_gitlab_work",
      ),
      bitbucket: account(
        "nadia_acme",
        "Nadia Petrenko",
        "nadia@acme.example",
        "id_ed25519_bitbucket_work",
      ),
    },
    {
      id: P_OSS,
      name: "Open Source",
      default_platform: "github",
      is_active: false,
      github: account(
        "nadiacodes",
        "Nadia P.",
        "9128374+nadiacodes@users.noreply.github.com",
        "id_ed25519_github_oss",
      ),
    },
    {
      id: P_NW,
      name: "Client · Northwind",
      default_platform: "gitlab",
      is_active: false,
      gitlab: account(
        "nadia-nw",
        "Nadia Petrenko",
        "n.petrenko@northwind.example",
        "id_ed25519_gitlab_nw",
      ),
      bitbucket: account(
        "nadia-northwind",
        "Nadia Petrenko",
        "nadia@northwind.example",
        "id_ed25519_bitbucket_nw",
      ),
    },
  ];

  const SSH_KEYS = [
    key("id_ed25519_github_work"),
    key("id_ed25519_gitlab_work"),
    key("id_ed25519_bitbucket_work"),
    key("id_ed25519_github_oss"),
    key("id_ed25519_gitlab_nw"),
    key("id_ed25519_bitbucket_nw"),
    key("id_rsa"),
  ];

  const ROOTS = [
    {
      path: "D:/work/acme",
      profile_id: P_WORK,
      platform: "github",
      install_hook: true,
      pin_remote_alias: true,
    },
    {
      path: "D:/work/acme-labs",
      profile_id: P_WORK,
      platform: "gitlab",
      install_hook: true,
      pin_remote_alias: false,
    },
  ];

  const repo = (over) => ({
    suggested_profile_id: P_WORK,
    candidate_profile_ids: [],
    bound: true,
    install_hook: true,
    pin_remote_alias: true,
    overrides_root: false,
    ...over,
  });

  // Keyed by root path so a scan answers with what that folder holds.
  const DISCOVERED = {
    "D:/work/acme": [
      repo({
        path: "D:/work/acme/billing-api",
        name: "billing-api",
        root_path: "D:/work/acme",
        remote_url: "git@github-work:acme-corp/billing-api.git",
        host: "github-work",
        owner: "acme-corp",
        repo: "billing-api",
        suggested_platform: "github",
        reason: "alias",
      }),
      repo({
        path: "D:/work/acme/design-system",
        name: "design-system",
        root_path: "D:/work/acme",
        remote_url: "git@github.com:nadia-acme/design-system.git",
        host: "github.com",
        owner: "nadia-acme",
        repo: "design-system",
        suggested_platform: "github",
        reason: "owner",
      }),
      repo({
        path: "D:/work/acme/legacy-etl",
        name: "legacy-etl",
        root_path: "D:/work/acme",
        remote_url: "git@github-work:acme-corp/legacy-etl.git",
        host: "github-work",
        owner: "acme-corp",
        repo: "legacy-etl",
        suggested_platform: "github",
        reason: "alias",
      }),
      repo({
        path: "D:/work/acme/infra-terraform",
        name: "infra-terraform",
        root_path: "D:/work/acme",
        remote_url: "git@github.com:shared-tools/infra-terraform.git",
        host: "github.com",
        owner: "shared-tools",
        repo: "infra-terraform",
        suggested_profile_id: null,
        suggested_platform: "github",
        reason: "ambiguous",
        candidate_profile_ids: [P_WORK, P_NW],
        bound: false,
        install_hook: false,
        pin_remote_alias: false,
        overrides_root: true,
      }),
      repo({
        path: "D:/work/acme/sandbox-notes",
        name: "sandbox-notes",
        root_path: "D:/work/acme",
        remote_url: "https://github.com/hexlabs/sandbox-notes.git",
        host: "github.com",
        owner: "hexlabs",
        repo: "sandbox-notes",
        suggested_profile_id: null,
        suggested_platform: null,
        reason: "unknown",
        bound: false,
      }),
    ],
    "D:/work/acme-labs": [
      repo({
        path: "D:/work/acme-labs/runner-images",
        name: "runner-images",
        root_path: "D:/work/acme-labs",
        remote_url: "git@gitlab-work:acme-labs/runner-images.git",
        host: "gitlab-work",
        owner: "acme-labs",
        repo: "runner-images",
        suggested_platform: "gitlab",
        reason: "alias",
        pin_remote_alias: false,
      }),
      repo({
        path: "D:/work/acme-labs/metrics-agent",
        name: "metrics-agent",
        root_path: "D:/work/acme-labs",
        remote_url: "git@gitlab-work:acme-labs/metrics-agent.git",
        host: "gitlab-work",
        owner: "acme-labs",
        repo: "metrics-agent",
        suggested_platform: "gitlab",
        reason: "owner",
        pin_remote_alias: false,
      }),
    ],
  };

  const BINDINGS = Object.values(DISCOVERED)
    .flat()
    .filter((r) => r.bound)
    .map((r) => ({
      path: r.path,
      profile_id: P_WORK,
      platform: r.suggested_platform,
      pin_remote_alias: r.pin_remote_alias,
      install_hook: r.install_hook,
      extra_allowed_emails: [],
      overrides_root: r.overrides_root,
    }));

  const WORK_NOREPLY = "48213574+nadia-acme@users.noreply.github.com";

  const DOCTOR_REPOS = [
    {
      path: "D:/work/acme/design-system",
      name: "design-system",
      profile_id: P_WORK,
      profile_name: "Work",
      platform: "github",
      expected_email: WORK_NOREPLY,
      effective_email: WORK_NOREPLY,
      remote_url: "https://github.com/nadia-acme/design-system.git",
      offending_emails: [],
      ok: false,
      checks: [
        { id: "exists", ok: true, detail: "D:/work/acme/design-system" },
        { id: "identity", ok: true, detail: WORK_NOREPLY },
        { id: "local", ok: true, detail: WORK_NOREPLY },
        {
          id: "remote",
          ok: false,
          detail: "HTTPS remote — the SSH alias is not pinned",
        },
        { id: "history", ok: true, detail: "clean" },
        { id: "hooks", ok: false, detail: "kept-existing" },
      ],
    },
    {
      path: "D:/work/acme-labs/runner-images",
      name: "runner-images",
      profile_id: P_WORK,
      profile_name: "Work",
      platform: "gitlab",
      expected_email: "n.petrenko@acme.example",
      effective_email: "nadia.p@mail.example",
      remote_url: "git@gitlab-work:acme-labs/runner-images.git",
      offending_emails: ["nadia.p@mail.example"],
      ok: false,
      checks: [
        { id: "exists", ok: true, detail: "D:/work/acme-labs/runner-images" },
        { id: "identity", ok: false, detail: "nadia.p@mail.example" },
        { id: "local", ok: false, detail: "no user.email in the repository" },
        { id: "remote", ok: true, detail: "gitlab-work:acme-labs/runner-images" },
        { id: "history", ok: true, detail: "clean" },
        { id: "hooks", ok: false, detail: "off" },
      ],
    },
    {
      path: "D:/work/acme/legacy-etl",
      name: "legacy-etl",
      profile_id: P_WORK,
      profile_name: "Work",
      platform: "github",
      expected_email: WORK_NOREPLY,
      effective_email: "nadia.p@mail.example",
      remote_url: "git@github-work:acme-corp/legacy-etl.git",
      offending_emails: ["nadia.p@mail.example"],
      ok: false,
      checks: [
        { id: "exists", ok: true, detail: "D:/work/acme/legacy-etl" },
        { id: "identity", ok: false, detail: "nadia.p@mail.example" },
        { id: "local", ok: false, detail: "no user.email in the repository" },
        { id: "remote", ok: true, detail: "github-work:acme-corp/legacy-etl" },
        { id: "history", ok: false, detail: "3 commits by nadia.p@mail.example" },
        { id: "hooks", ok: false, detail: "missing" },
      ],
    },
    {
      path: "D:/work/acme/billing-api",
      name: "billing-api",
      profile_id: P_WORK,
      profile_name: "Work",
      platform: "github",
      expected_email: WORK_NOREPLY,
      effective_email: WORK_NOREPLY,
      remote_url: "git@github-work:acme-corp/billing-api.git",
      offending_emails: [],
      ok: true,
      checks: [
        { id: "exists", ok: true, detail: "D:/work/acme/billing-api" },
        { id: "identity", ok: true, detail: WORK_NOREPLY },
        { id: "local", ok: true, detail: WORK_NOREPLY },
        { id: "remote", ok: true, detail: "github-work:acme-corp/billing-api" },
        { id: "history", ok: true, detail: "clean" },
        { id: "hooks", ok: true, detail: "installed" },
      ],
    },
  ];

  const GUARD = {
    unset_global_identity: true,
    manage_gitconfig_includes: true,
    own_bare_ssh_hosts: false,
  };

  const SETTINGS = {
    github_client_id: "Ov23liK7pQx4WmZ0aT2u",
    gitlab_client_id:
      "a1c9f0e6b74d2385c1fa9e70d4b62c85f3197ad0e6428bc51d93f70ae82c46b9",
    use_openssh_for_git_tools: true,
  };

  const handlers = {
    get_profiles: () => PROFILES,
    get_git_identity: () => ({ name: "", email: "" }),
    save_profile: () => null,
    delete_profile: () => null,
    activate_profile: () => null,
    list_ssh_keys: () => SSH_KEYS,
    read_public_key: () => "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI…",
    delete_ssh_keys: () => null,
    delete_platform_token: () => null,
    delete_profile_tokens: () => null,
    get_settings: () => SETTINGS,
    save_settings: () => null,
    save_guard_settings: () => null,
    openssh_integration_probe: () => ({
      available: true,
      ssh_exe: "C:\\Windows\\System32\\OpenSSH\\ssh.exe",
    }),
    get_repo_state: () => ({ roots: ROOTS, bindings: BINDINGS, guard: GUARD }),
    doctor: () => ({
      guard: {
        global_name: "",
        global_email: "",
        use_config_only: true,
        includes_managed: true,
        gitconfig_path: `${HOME}/.gitconfig`,
        ok: true,
      },
      repos: DOCTOR_REPOS,
    }),
    scan_profile_repositories: ({ roots }) =>
      (roots ?? []).flatMap((r) => DISCOVERED[r.path] ?? []),
    apply_profile_repos: () => ({ bound: 0, released: 0, failed: [] }),
    set_tray_labels: () => null,
    // The device flow is shown mid-flight: the code appears, the poll never
    // resolves to a user, so the panel stays on screen for the capture.
    github_oauth_start: () => ({
      device_code: "b8c0f2d1a4e6931752ac0de4f8b21c9037ae65d1",
      user_code: "WDJB-MJHT",
      verification_uri: "https://github.com/login/device",
      expires_in: 899,
      interval: 5,
    }),
    github_oauth_poll: () => null,
    "plugin:event|listen": () => 1,
    "plugin:event|unlisten": () => null,
    "plugin:updater|check": () => null,
    "plugin:autostart|is_enabled": () => true,
    "plugin:opener|open_url": () => null,
    "plugin:dialog|open": () => "D:/work/acme-labs",
    "plugin:clipboard-manager|write_text": () => null,
  };

  let nextCallbackId = 1;

  // React's StrictMode mounts twice, so every `listen()` is torn down once —
  // which goes through this object rather than through `invoke`.
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

  window.__TAURI_INTERNALS__ = {
    transformCallback(callback) {
      const id = nextCallbackId++;
      window[`_${id}`] = callback;
      return id;
    },
    convertFileSrc: (path) => path,
    invoke(cmd, args) {
      const handler = handlers[cmd];
      if (!handler) {
        console.error(`[tauri-mock] unmocked command: ${cmd}`);
        return Promise.resolve(null);
      }
      return Promise.resolve(handler(args ?? {}));
    },
  };
})();
