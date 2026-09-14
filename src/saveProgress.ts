import type { SaveProgress } from "./types";

/**
 * Where the save's progress bar stands, in percent.
 *
 * Every folder owns an equal share of the bar and applying the rules owns one
 * share more. Inside a folder the share fills with the repositories that have
 * been read: how many there are is known only once the folder has been walked,
 * so the bar rests at the folder's own mark until the walk answers.
 */
export function savePercent(p: SaveProgress): number {
  const shares = p.folder_count + 1;
  const inFolder = p.repos_total > 0 ? p.repos_done / p.repos_total : 0;
  const done = p.stage === "apply" ? p.folder_count : p.folder_index + inFolder;
  return Math.max(0, Math.min(100, Math.round((done / shares) * 100)));
}
