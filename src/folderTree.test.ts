import { describe, expect, it } from "vitest";
import { treeRow } from "./folderTree";
import type { FolderRepo } from "./types";

function repo(
  relative: string,
  name = relative.split("/").pop() ?? "",
): FolderRepo {
  return {
    path: `D:/repos/work/${relative}`,
    relative,
    name,
    root_path: "D:/repos/work",
    remote_url: "git@github.com:octo/demo.git",
    full_name: "octo/demo",
    foreign_host: false,
  };
}

describe("treeRow", () => {
  it("indents by how deep the repository sits under the folder", () => {
    expect(treeRow(repo("demo"))).toEqual({
      depth: 0,
      prefix: "",
      label: "demo",
    });
    expect(treeRow(repo("group/demo"))).toEqual({
      depth: 1,
      prefix: "group",
      label: "demo",
    });
    expect(treeRow(repo("group/deep/demo"))).toEqual({
      depth: 2,
      prefix: "group/deep",
      label: "demo",
    });
  });

  // The watched folder can itself be a repository; the backend says so with ".",
  // and an indent of one for the folder's own row would read as nesting.
  it("puts the folder's own repository at the top level", () => {
    expect(treeRow(repo(".", "work"))).toEqual({
      depth: 0,
      prefix: "",
      label: "work",
    });
  });
});
