import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { fmt, useI18n } from "../i18n";
import { PLATFORM_LABEL, PLATFORMS, profileUrl } from "../platforms";
import type { PlatformAccount, PlatformId, Profile } from "../types";
import ConfirmDialog, { type DialogAction } from "./ConfirmDialog";
import { BitbucketIcon, GitHubIcon, GitLabIcon } from "./icons";

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

  // A div rather than a button because it contains one — the @username link —
  // and a button inside a button is invalid. Carrying the role means carrying
  // the keyboard behaviour a real button would have had; the two branches are
  // written out so the interactive one is unambiguously interactive rather
  // than a set of conditional attributes.
  const body = (
    <>
      <div className="flex items-center gap-2 text-sm text-fg-3">
        <Icon className="h-4 w-4 shrink-0 text-fg-4" />
        <span>
          {label}:{" "}
          <button
            type="button"
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
    </>
  );

  if (!canClick) return <div className={wrapper}>{body}</div>;

  return (
    // biome-ignore lint/a11y/useSemanticElements: contains the @username button, and a button inside a button is invalid
    <div
      className={wrapper}
      role="button"
      tabIndex={0}
      title={fmt(m.card.setDefaultTitle, { platform: label })}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
    >
      {body}
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
  const defaultP = profile.default_platform || connected[0] || "bitbucket";
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
              type="button"
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
            {connected.map((platform) => {
              // `connected` was built by filtering on this very field, but the
              // compiler cannot carry that across, and a `!` would hide a real
              // mismatch if the filter ever changed.
              const account = profile[platform];
              if (!account) return null;
              return (
                <PlatformBadge
                  key={platform}
                  platform={platform}
                  account={account}
                  isDefault={defaultP === platform}
                  canClick={canChooseDefault}
                  onClick={() => onSetDefault(profile.id, platform)}
                />
              );
            })}
          </div>
        )}

        <div className="flex gap-2">
          {!profile.is_active && (
            <button
              type="button"
              onClick={() => onActivate(profile.id)}
              className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-emerald-500"
            >
              {m.card.activate}
            </button>
          )}
          <button
            type="button"
            onClick={() => onEdit(profile)}
            className="rounded-md bg-subtle px-3 py-1.5 text-xs font-medium text-fg-2 transition-colors hover:bg-hover"
          >
            {m.card.edit}
          </button>
          <button
            type="button"
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
