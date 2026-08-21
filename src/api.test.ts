import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * The one thing a unit test can prove about the bridge to Rust: that both sides
 * still agree on the command names.
 *
 * Nothing else checks this. A command renamed in `lib.rs` leaves `api.ts`
 * compiling and typechecking, and the failure only surfaces as a rejected
 * promise on a screen someone has to reach by hand. Reading both files and
 * comparing the two lists is cheap and catches it at `pnpm test`.
 */

function read(relative: string): string {
  return readFileSync(
    fileURLToPath(new URL(relative, import.meta.url)),
    "utf8",
  );
}

/** Command names `api.ts` sends. */
function commandsCalledByFrontend(): string[] {
  const source = read("./api.ts");
  return [...source.matchAll(/invoke<[^>]*>\(\s*"([a-z0-9_]+)"/g)].map(
    (match) => match[1],
  );
}

/** Command names `generate_handler!` registers. */
function commandsRegisteredByBackend(): string[] {
  const source = read("../src-tauri/src/lib.rs");
  const block = source.match(/generate_handler!\[([\s\S]*?)\]/);
  if (!block) throw new Error("could not find generate_handler! in lib.rs");
  return [...block[1].matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]);
}

describe("the command bridge", () => {
  it("finds commands on both sides", () => {
    // Guards the regexes themselves: a parsing change that silently matched
    // nothing would make every assertion below pass for the wrong reason.
    expect(commandsCalledByFrontend().length).toBeGreaterThan(20);
    expect(commandsRegisteredByBackend().length).toBeGreaterThan(20);
  });

  it("sends no command the backend does not register", () => {
    const registered = new Set(commandsRegisteredByBackend());
    const unknown = commandsCalledByFrontend().filter(
      (name) => !registered.has(name),
    );
    expect(unknown).toEqual([]);
  });

  it("names each command once, so there is a single place to rename it", () => {
    const called = commandsCalledByFrontend();
    const duplicates = called.filter((n, i) => called.indexOf(n) !== i);
    expect([...new Set(duplicates)]).toEqual([]);
  });

  it("has no invoke call left outside the wrapper", () => {
    // The whole point of api.ts is that it is the only file naming a command.
    for (const file of [
      "./App.tsx",
      "./components/ProfileForm.tsx",
      "./components/ProfileRepos.tsx",
      "./components/SettingsPage.tsx",
    ]) {
      expect(read(file), file).not.toMatch(/\binvoke[<(]/);
    }
  });
});
