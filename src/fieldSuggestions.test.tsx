import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { noSuggestions } from "./fieldSuggestions";

describe("noSuggestions", () => {
  // What the webview reads is the element, not the object, so the element is
  // what gets asked: a key dropped from the object or a value that stops
  // saying "off" is invisible everywhere else.
  it("reaches the element as attributes the webview reads", () => {
    render(<input aria-label="field" {...noSuggestions} />);
    const field = screen.getByLabelText("field");
    expect(field).toHaveAttribute("autocomplete", "off");
    expect(field).toHaveAttribute("autocorrect", "off");
    expect(field).toHaveAttribute("autocapitalize", "off");
  });
});

/**
 * Vite rewrites `new URL("./a-literal", import.meta.url)` into a URL served by
 * the dev server, which is no longer a path any file can be read from. Handing
 * the specifier over as a variable leaves the call alone.
 */
function here(relative: string): string {
  return fileURLToPath(new URL(relative, import.meta.url));
}

/** The directory this test sits in, which is the app's source root. */
const SRC = dirname(here("./fieldSuggestions.ts"));

function tsxFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return tsxFiles(path);
    return entry.isFile() &&
      path.endsWith(".tsx") &&
      !path.endsWith(".test.tsx")
      ? [path]
      : [];
  });
}

/** Every `<input ... />` in a file, as the text between the tag and its close. */
function inputTags(source: string): string[] {
  return [...source.matchAll(/<input\b[\s\S]*?\/>/g)].map((match) => match[0]);
}

describe("text fields", () => {
  // The point of one shared object is that the next field added spreads it too,
  // which nothing but this enforces: a field without it looks right on screen
  // and only misbehaves once the webview has a value to offer.
  it("every field that takes typing turns the webview's suggestions off", () => {
    const missing: string[] = [];
    let checked = 0;
    for (const file of tsxFiles(SRC)) {
      for (const tag of inputTags(readFileSync(file, "utf8"))) {
        // A checkbox has nothing to suggest.
        if (tag.includes('type="checkbox"')) continue;
        checked++;
        if (!tag.includes("{...noSuggestions}")) {
          missing.push(`${file}: ${tag.split("\n")[1]?.trim() ?? tag}`);
        }
      }
    }
    expect(checked, "the scan found no text fields at all").toBeGreaterThan(4);
    expect(missing).toEqual([]);
  });
});
