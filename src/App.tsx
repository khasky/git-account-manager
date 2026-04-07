import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Profile } from "./types";
import { useTheme } from "./ThemeContext";
import ProfileCard from "./components/ProfileCard";
import ProfileForm from "./components/ProfileForm";
import SettingsPage from "./components/SettingsPage";

type View = "list" | "form" | "settings";

const SunIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
    />
  </svg>
);

const MoonIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
    />
  </svg>
);

const MonitorIcon = () => (
  <svg
    className="h-4 w-4"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
    strokeWidth={2}
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
    />
  </svg>
);

function App() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [view, setView] = useState<View>("list");
  const [editingProfile, setEditingProfile] = useState<Profile | null>(null);
  const [loading, setLoading] = useState(true);
  const [toastMsg, setToastMsg] = useState("");
  const { preference, setPreference } = useTheme();
  const [themeOpen, setThemeOpen] = useState(false);
  const themeRef = useRef<HTMLDivElement>(null);

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
      const data = await invoke<Profile[]>("get_profiles");
      setProfiles(data);
    } catch (e) {
      console.error("Failed to load profiles:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  function showToast(msg: string) {
    setToastMsg(msg);
    setTimeout(() => setToastMsg(""), 3000);
  }

  function handleAdd() {
    setEditingProfile(null);
    setView("form");
  }

  function handleEdit(profile: Profile) {
    setEditingProfile(profile);
    setView("form");
  }

  async function handleActivate(id: string) {
    try {
      await invoke("activate_profile", { id });
      await loadProfiles();
      const p = profiles.find((pr) => pr.id === id);
      showToast(`Activated: ${p?.name || "profile"}`);
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  }

  async function handleDelete(id: string, deleteKeys: boolean) {
    const profile = profiles.find((p) => p.id === id);
    if (!profile) return;
    try {
      if (deleteKeys) {
        const keyPaths: string[] = [];
        if (profile.github) {
          keyPaths.push(profile.github.ssh_private_key_path);
          if (profile.github.token && profile.github.ssh_public_key_path) {
            await invoke("remove_ssh_key_from_platform", {
              platform: "github",
              token: profile.github.token,
              publicKeyPath: profile.github.ssh_public_key_path,
            }).catch(() => {});
          }
        }
        if (profile.gitlab) {
          keyPaths.push(profile.gitlab.ssh_private_key_path);
          if (profile.gitlab.token && profile.gitlab.ssh_public_key_path) {
            await invoke("remove_ssh_key_from_platform", {
              platform: "gitlab",
              token: profile.gitlab.token,
              publicKeyPath: profile.gitlab.ssh_public_key_path,
            }).catch(() => {});
          }
        }
        if (keyPaths.length > 0) {
          await invoke("delete_ssh_keys", { paths: keyPaths });
        }
      }
      await invoke("delete_profile", { id });
      await loadProfiles();
      setView("list");
      showToast(`Deleted: ${profile.name}`);
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  }

  async function handleSetDefault(id: string, platform: "github" | "gitlab") {
    const profile = profiles.find((p) => p.id === id);
    if (!profile) return;
    try {
      await invoke("save_profile", {
        profile: { ...profile, default_platform: platform },
      });
      await loadProfiles();
      showToast(
        `Default identity: ${platform === "github" ? "GitHub" : "GitLab"}`,
      );
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  }

  async function handleSave(_profile: Profile) {
    await loadProfiles();
    setView("list");
    showToast("Profile saved");
  }

  const themeOptions = [
    { value: "light" as const, label: "Light", icon: <SunIcon /> },
    { value: "dark" as const, label: "Dark", icon: <MoonIcon /> },
    { value: "system" as const, label: "System", icon: <MonitorIcon /> },
  ];

  const currentIcon =
    preference === "light" ? (
      <SunIcon />
    ) : preference === "dark" ? (
      <MoonIcon />
    ) : (
      <MonitorIcon />
    );

  if (view === "form") {
    return (
      <div className="flex h-screen flex-col bg-surface text-fg">
        <ProfileForm
          profile={editingProfile}
          onSave={handleSave}
          onCancel={() => setView("list")}
          onSettings={() => setView("settings")}
          onDelete={handleDelete}
        />
      </div>
    );
  }

  if (view === "settings") {
    return (
      <div className="flex h-screen flex-col bg-surface text-fg">
        <SettingsPage onBack={() => setView("list")} />
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-surface text-fg">
      <header className="flex items-center justify-between border-b border-bd px-6 py-4">
        <div>
          <h1 className="text-xl font-bold text-fg">Git Account Manager</h1>
          <p className="text-xs text-fg-4">
            Manage SSH keys, git identity, and platform accounts
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() =>
              openUrl("https://github.com/khasky/git-account-manager")
            }
            className="rounded-md bg-raised p-2 text-fg-4 transition-colors hover:bg-subtle hover:text-fg-2"
            title="GitHub Repository"
          >
            <svg className="h-5 w-5" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z" />
            </svg>
          </button>
          <div className="relative" ref={themeRef}>
            <button
              onClick={() => setThemeOpen((v) => !v)}
              className="rounded-md bg-raised p-2 text-fg-4 transition-colors hover:bg-subtle hover:text-fg-2"
              title="Theme"
            >
              {currentIcon}
            </button>
            {themeOpen && (
              <div className="absolute right-0 z-50 mt-1 w-36 rounded-lg border border-bd bg-dialog py-1 shadow-lg">
                {themeOptions.map((opt) => (
                  <button
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
            onClick={() => setView("settings")}
            className="rounded-md bg-raised p-2 text-fg-4 transition-colors hover:bg-subtle hover:text-fg-2"
            title="OAuth Settings"
          >
            <svg
              className="h-5 w-5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1.5}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"
              />
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"
              />
            </svg>
          </button>
          <button
            onClick={handleAdd}
            className="flex items-center gap-1.5 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500"
          >
            <svg
              className="h-4 w-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 4v16m8-8H4"
              />
            </svg>
            New Profile
          </button>
        </div>
      </header>

      <main className="flex-1 overflow-y-auto p-6">
        {loading ? (
          <div className="flex items-center justify-center py-20 text-fg-4">
            Loading...
          </div>
        ) : profiles.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20">
            <svg
              className="mb-4 h-16 w-16 text-fg-6"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={1}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
            <p className="mb-2 text-lg text-fg-4">No profiles yet</p>
            <p className="mb-4 text-sm text-fg-5">
              Create your first profile to get started
            </p>
            <button
              onClick={handleAdd}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500"
            >
              Create Profile
            </button>
          </div>
        ) : (
          <div className="mx-auto max-w-2xl space-y-3">
            {profiles.map((p) => (
              <ProfileCard
                key={p.id}
                profile={p}
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
        <div className="fixed right-4 bottom-4 rounded-lg border border-bd bg-raised px-4 py-2 text-sm text-fg-2 shadow-lg">
          {toastMsg}
        </div>
      )}
    </div>
  );
}

export default App;
