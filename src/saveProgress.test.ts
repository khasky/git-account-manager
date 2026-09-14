import { describe, expect, it } from "vitest";
import { savePercent } from "./saveProgress";
import type { SaveProgress } from "./types";

function progress(patch: Partial<SaveProgress>): SaveProgress {
  return {
    stage: "scan",
    folder: "D:/repos/work",
    folder_index: 0,
    folder_count: 1,
    repos_done: 0,
    repos_total: 0,
    ...patch,
  };
}

describe("savePercent", () => {
  it("rests at the folder's own mark while its walk is still counting", () => {
    expect(savePercent(progress({ folder_index: 0, folder_count: 2 }))).toBe(0);
    expect(savePercent(progress({ folder_index: 1, folder_count: 2 }))).toBe(
      33,
    );
  });

  it("fills the folder's share with the repositories read", () => {
    expect(
      savePercent(
        progress({ folder_count: 1, repos_done: 11, repos_total: 22 }),
      ),
    ).toBe(25);
    expect(
      savePercent(
        progress({ folder_count: 1, repos_done: 22, repos_total: 22 }),
      ),
    ).toBe(50);
  });

  it("leaves the last share to applying the rules", () => {
    expect(
      savePercent(
        progress({ stage: "apply", folder_index: 1, folder_count: 1 }),
      ),
    ).toBe(50);
    expect(
      savePercent(
        progress({ stage: "apply", folder_index: 3, folder_count: 3 }),
      ),
    ).toBe(75);
  });
});
