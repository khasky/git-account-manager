import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./api";
import FolderAlert from "./components/FolderAlert";
import {
  GearIcon,
  GitHubIcon,
  MonitorIcon,
  MoonIcon,
  PeopleIcon,
  PlusIcon,
  SunIcon,
} from "./components/icons";
import ProfileCard from "./components/ProfileCard";
import ProfileForm from "./components/ProfileForm";
import SettingsPage from "./components/SettingsPage";
import UpdateBanner from "./components/UpdateBanner";
import { fmt, useI18n } from "./i18n";
import { PLATFORM_LABEL, PLATFORMS } from "./platforms";
import { useTheme } from "./ThemeContext";
import type { FolderWatch, GitIdentity, PlatformId, Profile } from "./types";

type View = "list" | "form" | "settings";

const TOAST_MS = 3000;

/** How often the folders are checked while the window is open. Three file reads
 *  per folder, so the cost is negligible; the point is that a folder someone
 *  moved in Explorer surfaces here rather than on the next wrong commit. */
const WATCH_MS = 60_000;

function App() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [view, setView] = useState<View>("list");
  const [editingProfile, setEditingProfile] = useState<Profile | null>(null);
  const [importPrefill, setImportPrefill] = useState<GitIdentity | null>(null);
  const [loading, setLoading] = useState(true);
  const [problems, setProblems] = useState<Record<string, number>>({});
  const [watched, setWatched] = useState<FolderWatch[]>([]);
  const [alertOpen, setAlertOpen] = useState(false);
  const [alertBusy, setAlertBusy] = useState("");
  // What has already been raised, so a problem the user chose to leave alone
  // does not reopen the dialog every minute.
  const announcedRef = useRef<Set<string>>(new Set());
  const [toastMsg, setToastMsg] = useState("");
  const { preference, setPreference } = useTheme();
  const { m } = useI18n();
  const [themeOpen, setThemeOpen] = useState(false);
  const themeRef = useRef<HTMLDivElement>(null);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (themeRef.current && !themeRef.current.contains(e.target as Node))
        setThemeOpen(false);
    }
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      setProfiles(await api.getProfiles());
    } catch (e) {
      console.error("Failed to load profiles:", e);
    } finally {
      setLoading(false);
    }

    // The per-profile card shows how many of its folders stopped holding, so
    // the report is fetched here rather than only inside the form a user might
    // never open. It walks each folder, so it is deliberately not awaited
    // before the list renders — a count arriving a moment later beats a window
    // that shows nothing until Git has been asked.
    try {
      const report = await api.doctor();
      const counts: Record<string, number> = {};
      for (const folder of report.folders) {
        if (!folder.ok) {
          counts[folder.profile_id] = (counts[folder.profile_id] ?? 0) + 1;
        }
      }
      setProblems(counts);
    } catch (e) {
      console.error("Failed to read folder health:", e);
    }
  }, []);

  // A folder that stops matching its rule is not something the user goes
  // looking for, so the window comes forward for it. Only for a problem that
  // was not already raised: the same one every minute would be noise, and noise
  // is how a real one gets dismissed unread.
  const checkFolders = useCallback(async () => {
    let rows: FolderWatch[];
    try {
      rows = await api.watchFolders();
    } catch (e) {
      console.error("Failed to check folders:", e);
      return;
    }
    const broken = rows.filter((r) => r.state !== "ok");

    // Where a missing folder went is a disk search, so it is asked per finding
    // rather than on every tick of the timer that produced the finding.
    const located = await Promise.all(
      broken.map(async (folder) =>
        folder.state === "missing"
          ? {
              ...folder,
              moved_to: await api.locateFolder(folder.path).catch(() => null),
            }
          : folder,
      ),
    );
    setWatched(located);

    const keys = located.map((r) => `${r.path}|${r.state}`);
    const fresh = keys.filter((k) => !announcedRef.current.has(k));
    announcedRef.current = new Set(keys);
    if (fresh.length > 0) {
      setAlertOpen(true);
      api.focusWindow().catch(() => {});
    }
  }, []);

  useEffect(() => {
    checkFolders();
    const timer = setInterval(checkFolders, WATCH_MS);
    return () => clearInterval(timer);
  }, [checkFolders]);

  async function runAlertAction(key: string, task: () => Promise<unknown>) {
    setAlertBusy(key);
    try {
      await task();
      await loadProfiles();
      await checkFolders();
    } catch (e) {
      showToast(fmt(m.app.toastError, { error: String(e) }));
    } finally {
      setAlertBusy("");
    }
  }

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  // Localize the system-tray menu to match the selected interface language.
  useEffect(() => {
    api
      .setTrayLabels({
        show: m.tray.show,
        quit: m.tray.quit,
        activePrefix: m.tray.activePrefix,
        noActive: m.tray.noActiveProfile,
      })
      .catch((e) => console.error("Failed to localize tray menu:", e));
  }, [m.tray.show, m.tray.quit, m.tray.activePrefix, m.tray.noActiveProfile]);

  // The tray menu can switch the active profile from outside the window; reload
  // the list when the backend signals a change so the UI stays in sync.
  useEffect(() => {
    const unlisten = listen("profiles-changed", () => {
      loadProfiles();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [loadProfiles]);

  useEffect(() => {
    return () => {
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    };
  }, []);

  // Restart the timer rather than add one: a second toast arriving early would
  // otherwise be cleared by the first one's timeout.
  function showToast(msg: string) {
    if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    setToastMsg(msg);
    toastTimerRef.current = setTimeout(() => {
      setToastMsg("");
      toastTimerRef.current = null;
    }, TOAST_MS);
  }

  function handleAdd() {
    setImportPrefill(null);
    setEditingProfile(null);
    setView("form");
  }

  function handleEdit(profile: Profile) {
    setImportPrefill(null);
    setEditingProfile(profile);
    setView("form");
  }

  async function handleImportFromGit() {
    try {
      setImportPrefill(await api.getGitIdentity());
    } catch {
      setImportPrefill({ name: "", email: "" });
    }
    setEditingProfile(null);
    setView("form");
  }

  async function handleActivate(id: string) {
    // Read the name before reloading: after the reload this closure still holds
    // the old list, and reading it there only worked by accident.
    const name = profiles.find((p) => p.id === id)?.name;
    try {
      await api.activateProfile(id);
      await loadProfiles();
      showToast(
        fmt(m.app.toastActivated, { name: name || m.app.toastProfileFallback }),
      );
    } catch (e) {
      showToast(fmt(m.app.toastError, { error: String(e) }));
    }
  }

  async function handleDelete(id: string, deleteKeys: boolean) {
    const profile = profiles.find((p) => p.id === id);
    if (!profile) return;
    try {
      if (deleteKeys) {
        const keyPaths: string[] = [];
        for (const platform of PLATFORMS) {
          const account = profile[platform];
          if (!account) continue;
          keyPaths.push(account.ssh_private_key_path);
          if (account.ssh_public_key_path) {
            await api
              .removeSshKeyFromPlatform({
                platform,
                profileId: profile.id,
                publicKeyPath: account.ssh_public_key_path,
              })
              .catch(() => {});
          }
        }
        if (keyPaths.length > 0) await api.deleteSshKeys(keyPaths);
      }
      await api.deleteProfile(id);
      await loadProfiles();
      setView("list");
      showToast(fmt(m.app.toastDeleted, { name: profile.name }));
    } catch (e) {
      showToast(fmt(m.app.toastError, { error: String(e) }));
    }
  }

  async function handleSetDefault(id: string, platform: PlatformId) {
    const profile = profiles.find((p) => p.id === id);
    if (!profile) return;
    try {
      await api.saveProfile({ ...profile, default_platform: platform });
      await loadProfiles();
      showToast(
        fmt(m.app.toastDefaultIdentity, { platform: PLATFORM_LABEL[platform] }),
      );
    } catch (e) {
      showToast(fmt(m.app.toastError, { error: String(e) }));
    }
  }

  async function handleSave() {
    await loadProfiles();
    setView("list");
    showToast(m.app.toastProfileSaved);
  }

  const themeOptions = [
    { value: "light" as const, label: m.theme.light, icon: <SunIcon /> },
    { value: "dark" as const, label: m.theme.dark, icon: <MoonIcon /> },
    { value: "system" as const, label: m.theme.system, icon: <MonitorIcon /> },
  ];

  const currentIcon =
    preference === "light" ? (
      <SunIcon />
    ) : preference === "dark" ? (
      <MoonIcon />
    ) : (
      <MonitorIcon />
    );

  // Every view sits in the same full-height column; only the contents differ.
  // The folder alert rides along with all of them: a folder that moved is worth
  // saying whichever page happens to be open.
  const shell = (children: React.ReactNode) => (
    <div className="flex h-screen flex-col bg-surface text-fg">
      {children}
      {alertOpen && (
        <FolderAlert
          problems={watched}
          busy={alertBusy}
          onRelink={(path, newPath) =>
            runAlertAction(`relink:${path}`, () =>
              api.relinkFolder({ path, newPath }),
            )
          }
          onForget={(path) =>
            runAlertAction(`forget:${path}`, () => api.forgetFolder(path))
          }
          onOpenProfile={(profileId) => {
            const profile = profiles.find((p) => p.id === profileId);
            setAlertOpen(false);
            if (profile) handleEdit(profile);
          }}
          onDismiss={() => setAlertOpen(false)}
        />
      )}
    </div>
  );

  if (view === "form") {
    return shell(
      <ProfileForm
        profile={editingProfile}
        prefill={importPrefill}
        onSave={handleSave}
        onCancel={() => setView("list")}
        onSettings={() => setView("settings")}
        onDelete={handleDelete}
      />,
    );
  }

  if (view === "settings") {
    return shell(<SettingsPage onBack={() => setView("list")} />);
  }

  return shell(
    <>
      <UpdateBanner />
      <header className="flex items-center justify-between border-b border-bd px-6 py-4">
        <div>
          <h1 className="text-xl font-bold text-fg">Git Account Manager</h1>
          <p className="text-xs text-fg-4">{m.app.subtitle}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() =>
              openUrl("https://github.com/khasky/git-account-manager")
            }
            className="btn-icon"
            title={m.app.githubRepo}
          >
            <GitHubIcon className="h-5 w-5" />
          </button>
          <div className="relative" ref={themeRef}>
            <button
              type="button"
              onClick={() => setThemeOpen((v) => !v)}
              className="btn-icon"
              title={m.app.themeTitle}
              aria-expanded={themeOpen}
            >
              {currentIcon}
            </button>
            {themeOpen && (
              <div className="absolute right-0 z-50 mt-1 w-36 rounded-lg border border-bd bg-dialog py-1 shadow-lg">
                {themeOptions.map((opt) => (
                  <button
                    type="button"
                    key={opt.value}
                    onClick={() => {
                      setPreference(opt.value);
                      setThemeOpen(false);
                    }}
                    className={`flex w-full items-center gap-2 px-3 py-2 text-sm transition-colors ${
                      preference === opt.value
                        ? "bg-selected-bg text-selected-fg"
                        : "text-fg-3 hover:bg-raised"
                    }`}
                  >
                    {opt.icon}
                    {opt.label}
                  </button>
                ))}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={() => setView("settings")}
            className="btn-icon"
            title={m.app.oauthSettings}
          >
            <GearIcon />
          </button>
          <button
            type="button"
            onClick={handleAdd}
            className="btn-primary flex items-center gap-1.5"
          >
            <PlusIcon />
            {m.app.newProfile}
          </button>
        </div>
      </header>

      <main className="flex-1 overflow-y-auto p-6">
        {loading ? (
          <div className="flex items-center justify-center py-20 text-fg-4">
            {m.app.loading}
          </div>
        ) : profiles.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20">
            <PeopleIcon className="mb-4 h-16 w-16 text-fg-6" />
            <p className="mb-2 text-lg text-fg-4">{m.app.noProfiles}</p>
            <p className="mb-4 text-sm text-fg-5">{m.app.noProfilesHint}</p>
            <button type="button" onClick={handleAdd} className="btn-primary">
              {m.app.createProfile}
            </button>
            <button
              type="button"
              onClick={handleImportFromGit}
              className="mt-3 text-sm text-link hover:text-link-hover hover:underline"
            >
              {m.form.importFromGit}
            </button>
          </div>
        ) : (
          <div className="mx-auto max-w-2xl space-y-3">
            {profiles.map((p) => (
              <ProfileCard
                key={p.id}
                profile={p}
                problemCount={problems[p.id] ?? 0}
                onActivate={handleActivate}
                onEdit={handleEdit}
                onDelete={handleDelete}
                onSetDefault={handleSetDefault}
              />
            ))}
          </div>
        )}
      </main>

      {toastMsg && (
        <output className="fixed right-4 bottom-4 rounded-lg border border-bd bg-raised px-4 py-2 text-sm text-fg-2 shadow-lg">
          {toastMsg}
        </output>
      )}
    </>,
  );
}

export default App;
