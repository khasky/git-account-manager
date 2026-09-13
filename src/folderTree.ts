import type { FolderRepo } from "./types";

/** One row of the hierarchy a folder rule covers. */
export interface TreeRow {
  /** How far to indent: the number of directories between it and the folder. */
  depth: number;
  /** The directories above it, without a trailing slash, or empty at the top. */
  prefix: string;
  /** The directory the repository itself is in. */
  label: string;
}

/**
 * Where a repository sits under its watched folder.
 *
 * The rule covers the folder, so the list exists to show what that means, and a
 * flat column of absolute paths does not: a repository nested three levels down
 * has to read as nested. The backend hands back a relative path, and "." is the
 * watched folder itself being a repository.
 */
export function treeRow(repo: FolderRepo): TreeRow {
  const parts =
    repo.relative === "." || repo.relative === ""
      ? []
      : repo.relative.split("/").filter(Boolean);
  if (parts.length === 0) {
    return { depth: 0, prefix: "", label: repo.name };
  }
  return {
    depth: parts.length - 1,
    prefix: parts.slice(0, -1).join("/"),
    label: parts[parts.length - 1],
  };
}
