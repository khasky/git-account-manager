import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Profile, PlatformAccount } from "../types";
import ConfirmDialog, { DialogAction } from "./ConfirmDialog";

interface Props {
  profile: Profile;
  onActivate: (id: string) => void;
  onEdit: (profile: Profile) => void;
  onDelete: (id: string, deleteKeys: boolean) => void;
  onSetDefault: (id: string, platform: "github" | "gitlab") => void;
}

function PlatformBadge({
  label,
  icon,
  account,
  platform,
  isDefault,
  canClick,
  onClick,
}: {
  label: string;
  icon: React.ReactNode;
  account?: PlatformAccount;
  platform: "github" | "gitlab";
  isDefault: boolean;
  canClick: boolean;
  onClick: () => void;
}) {
  if (!account) {
    return (
      <div className="flex items-center gap-2 rounded-md border border-transparent px-3 py-2 text-sm text-fg-5">
        {icon}
        <span>
          {label}: <span className="italic">not connected</span>
        </span>
      </div>
    );
  }
  const profileUrl =
    platform === "github"
      ? `https://github.com/${account.username}`
      : `https://gitlab.com/${account.username}`;

  const wrapper = canClick
    ? `rounded-md border px-3 py-2 transition-colors ${
        isDefault
          ? "border-selected-border bg-selected-bg"
          : "border-bd bg-raised-40 hover:border-bd-s"
      }`
    : "rounded-md border border-transparent px-3 py-2";

  return (
    <div
      className={wrapper}
      onClick={canClick ? onClick : undefined}
      role={canClick ? "button" : undefined}
      title={canClick ? `Set ${label} as default git identity` : undefined}
    >
      <div className="flex items-center gap-2 text-sm text-fg-3">
        {icon}
        <span>
          {label}:{" "}
          <button
            onClick={(e) => {
              e.stopPropagation();
              openUrl(profileUrl);
            }}
            className="font-medium text-link hover:text-link-hover hover:underline"
          >
            @{account.username}
          </button>
        </span>
        {isDefault && canClick && (
          <span className="ml-auto text-[10px] font-medium text-link">
            default
          </span>
        )}
      </div>
      <div className="pl-6 text-xs text-fg-4">
        {account.git_name} &lt;{account.git_email}&gt;
      </div>
    </div>
  );
}

function keyFileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

function collectKeys(profile: Profile): string[] {
  const keys: string[] = [];
  if (profile.github?.ssh_private_key_path)
    keys.push(profile.github.ssh_private_key_path);
  if (profile.gitlab?.ssh_private_key_path)
    keys.push(profile.gitlab.ssh_private_key_path);
  return keys;
}

export default function ProfileCard({
  profile,
  onActivate,
  onEdit,
  onDelete,
  onSetDefault,
}: Props) {
  const defaultP =
    profile.default_platform || (profile.github ? "github" : "gitlab");
  const hasBoth = !!profile.github && !!profile.gitlab;
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);

  const keys = collectKeys(profile);

  const deleteActions: DialogAction[] = [
    ...(keys.length > 0
      ? [
          {
            label: "Delete profile and SSH keys",
            variant: "danger" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, true);
            },
          },
          {
            label: "Delete profile, keep SSH keys",
            variant: "default" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, false);
            },
          },
        ]
      : [
          {
            label: "Delete profile",
            variant: "danger" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, false);
            },
          },
        ]),
    {
      label: "Cancel",
      variant: "cancel" as const,
      onClick: () => setShowDeleteDialog(false),
    },
  ];

  return (
    <>
      <div
        className={`rounded-lg border p-4 transition-colors ${
          profile.is_active
            ? "border-active-border bg-active-bg"
            : "border-bd bg-raised-60 hover:border-bd-s"
        }`}
      >
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h3 className="text-lg font-semibold text-fg">{profile.name}</h3>
            {profile.is_active && (
              <span className="rounded-full bg-emerald-600/80 px-2 py-0.5 text-xs font-medium text-emerald-100">
                Active
              </span>
            )}
          </div>
        </div>

        <div className="mb-3 space-y-1.5">
          <PlatformBadge
            label="GitHub"
            platform="github"
            account={profile.github}
            isDefault={defaultP === "github"}
            canClick={hasBoth}
            onClick={() => onSetDefault(profile.id, "github")}
            icon={
              <svg
                className="h-4 w-4 shrink-0 text-fg-4"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z" />
              </svg>
            }
          />
          <PlatformBadge
            label="GitLab"
            platform="gitlab"
            account={profile.gitlab}
            isDefault={defaultP === "gitlab"}
            canClick={hasBoth}
            onClick={() => onSetDefault(profile.id, "gitlab")}
            icon={
              <svg
                className="h-4 w-4 shrink-0 text-fg-4"
                viewBox="80 85 220 200"
                fill="currentColor"
              >
                <path d="M282.83 170.73l-.27-.69-26.14-68.22a6.81 6.81 0 00-2.69-3.24 7 7 0 00-8 .43 7 7 0 00-2.32 3.52l-17.65 54h-71.47l-17.65-54a6.86 6.86 0 00-2.32-3.53 7 7 0 00-8-.43 6.87 6.87 0 00-2.69 3.24L97.44 170l-.26.69a48.54 48.54 0 0016.1 56.07l.09.07.24.17 39.82 29.82 19.7 14.91 12 9.06a8.07 8.07 0 009.76 0l12-9.06 19.7-14.91 40.06-30 .1-.08a48.56 48.56 0 0016.08-56.04z" />
              </svg>
            }
          />
        </div>

        <div className="flex gap-2">
          {!profile.is_active && (
            <button
              onClick={() => onActivate(profile.id)}
              className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-emerald-500"
            >
              Activate
            </button>
          )}
          <button
            onClick={() => onEdit(profile)}
            className="rounded-md bg-subtle px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-hover"
          >
            Edit
          </button>
          <button
            onClick={() => setShowDeleteDialog(true)}
            className="rounded-md bg-subtle px-3 py-1.5 text-xs font-medium text-danger-fg transition-colors hover:bg-danger-hover"
          >
            Delete
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={showDeleteDialog}
        title={`Delete profile "${profile.name}"?`}
        actions={deleteActions}
      >
        <p className="mb-3 text-sm text-fg-3">This action cannot be undone.</p>
        {keys.length > 0 && (
          <div className="space-y-1">
            <p className="text-xs text-fg-4">Associated SSH keys:</p>
            {keys.map((k) => (
              <div
                key={k}
                className="rounded bg-raised px-2 py-1 font-mono text-xs text-fg-3"
              >
                {keyFileName(k)}
              </div>
            ))}
          </div>
        )}
      </ConfirmDialog>
    </>
  );
}
