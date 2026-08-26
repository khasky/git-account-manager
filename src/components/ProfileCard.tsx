import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Profile, PlatformAccount, PlatformId } from "../types";
import { PLATFORMS, PLATFORM_LABEL, profileUrl } from "../platforms";
import ConfirmDialog, { DialogAction } from "./ConfirmDialog";
import { BitbucketIcon, GitHubIcon, GitLabIcon } from "./icons";
import { useI18n, fmt } from "../i18n";

interface Props {
  profile: Profile;
  /** Repositories of this profile the doctor found drifted. */
  problemCount: number;
  onActivate: (id: string) => void;
  onEdit: (profile: Profile) => void;
  onDelete: (id: string, deleteKeys: boolean) => void;
  onSetDefault: (id: string, platform: PlatformId) => void;
}

const PLATFORM_ICON: Record<
  PlatformId,
  ({ className }: { className: string }) => React.ReactElement
> = {
  github: GitHubIcon,
  gitlab: GitLabIcon,
  bitbucket: BitbucketIcon,
};

function PlatformBadge({
  account,
  platform,
  isDefault,
  canClick,
  onClick,
}: {
  account: PlatformAccount;
  platform: PlatformId;
  isDefault: boolean;
  canClick: boolean;
  onClick: () => void;
}) {
  const { m } = useI18n();
  const label = PLATFORM_LABEL[platform];
  const Icon = PLATFORM_ICON[platform];

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
      title={
        canClick ? fmt(m.card.setDefaultTitle, { platform: label }) : undefined
      }
    >
      <div className="flex items-center gap-2 text-sm text-fg-3">
        <Icon className="h-4 w-4 shrink-0 text-fg-4" />
        <span>
          {label}:{" "}
          <button
            onClick={(e) => {
              e.stopPropagation();
              openUrl(profileUrl(platform, account.username));
            }}
            className="font-medium text-link hover:text-link-hover hover:underline"
          >
            @{account.username}
          </button>
        </span>
        {isDefault && canClick && (
          <span className="ml-auto text-[10px] font-medium text-link">
            {m.card.default}
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
  return PLATFORMS.map((p) => profile[p]?.ssh_private_key_path).filter(
    (path): path is string => Boolean(path),
  );
}

export default function ProfileCard({
  profile,
  problemCount,
  onActivate,
  onEdit,
  onDelete,
  onSetDefault,
}: Props) {
  const { m } = useI18n();
  const connected = PLATFORMS.filter((p) => profile[p]);
  const defaultP =
    profile.default_platform || connected[0] || "bitbucket";
  const canChooseDefault = connected.length >= 2;
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);

  const keys = collectKeys(profile);

  const deleteActions: DialogAction[] = [
    ...(keys.length > 0
      ? [
          {
            label: m.card.deleteAndKeys,
            variant: "danger" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, true);
            },
          },
          {
            label: m.card.deleteKeepKeys,
            variant: "default" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, false);
            },
          },
        ]
      : [
          {
            label: m.card.deleteProfile,
            variant: "danger" as const,
            onClick: () => {
              setShowDeleteDialog(false);
              onDelete(profile.id, false);
            },
          },
        ]),
    {
      label: m.card.cancel,
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
                {m.card.active}
              </span>
            )}
          </div>
          {problemCount > 0 && (
            <button
              onClick={() => onEdit(profile)}
              title={m.card.problemsTitle}
              className="rounded-full bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-700 hover:bg-amber-500/25 dark:text-amber-400"
            >
              {fmt(m.card.problems, { count: problemCount })}
            </button>
          )}
        </div>

        {connected.length > 0 && (
          <div className="mb-3 space-y-1.5">
            {connected.map((platform) => (
              <PlatformBadge
                key={platform}
                platform={platform}
                account={profile[platform]!}
                isDefault={defaultP === platform}
                canClick={canChooseDefault}
                onClick={() => onSetDefault(profile.id, platform)}
              />
            ))}
          </div>
        )}

        <div className="flex gap-2">
          {!profile.is_active && (
            <button
              onClick={() => onActivate(profile.id)}
              className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-emerald-500"
            >
              {m.card.activate}
            </button>
          )}
          <button
            onClick={() => onEdit(profile)}
            className="rounded-md bg-subtle px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-hover"
          >
            {m.card.edit}
          </button>
          <button
            onClick={() => setShowDeleteDialog(true)}
            className="rounded-md bg-subtle px-3 py-1.5 text-xs font-medium text-danger-fg transition-colors hover:bg-danger-hover"
          >
            {m.card.delete}
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={showDeleteDialog}
        title={fmt(m.card.deleteTitle, { name: profile.name })}
        actions={deleteActions}
      >
        <p className="mb-3 text-sm text-fg-3">{m.card.deleteBody}</p>
        {keys.length > 0 && (
          <div className="space-y-1">
            <p className="text-xs text-fg-4">{m.card.deleteKeysLabel}</p>
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
