import { describe, expect, it } from "vitest";
import { crateVersion, parityError } from "./tauri-parity.mjs";

// `tauri-build` sits before `tauri` here on purpose: an unanchored search finds
// it first and reads a version off the wrong crate.
const CARGO_LOCK = `[[package]]
name = "tauri-build"
version = "2.6.3"

[[package]]
name = "tauri"
version = "2.11.1"
`;

describe("crateVersion", () => {
  it("reads the crate without matching a longer name that starts the same", () => {
    expect(crateVersion(CARGO_LOCK)).toBe("2.11.1");
  });

  it("returns null when the crate is absent", () => {
    expect(crateVersion("[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n")).toBeNull();
  });
});

describe("parityError", () => {
  it("passes when major and minor agree and only the patch differs", () => {
    expect(parityError("2.11.1", "2.11.4")).toBeNull();
  });

  it("reports the pair the CLI rejects on a minor mismatch", () => {
    expect(parityError("2.11.1", "2.10.1")).toBe(
      "tauri (v2.11.1) : @tauri-apps/api (v2.10.1)",
    );
  });

  it("reports the pair the CLI rejects on a major mismatch", () => {
    expect(parityError("3.0.0", "2.11.1")).toBe(
      "tauri (v3.0.0) : @tauri-apps/api (v2.11.1)",
    );
  });

  it("names the missing crate rather than comparing against nothing", () => {
    expect(parityError(null, "2.11.1")).toBe(
      'no "tauri" entry in src-tauri/Cargo.lock',
    );
  });
});
