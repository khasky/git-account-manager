import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fmt, rich } from "./index";

describe("fmt", () => {
  it("substitutes every placeholder it was given", () => {
    expect(fmt("Activated: {name}", { name: "Work" })).toBe("Activated: Work");
    expect(fmt("{a} and {b}", { a: 1, b: 2 })).toBe("1 and 2");
  });

  // A translator can drop a placeholder, and a caller can forget one. Leaving
  // the token visible says which, where blanking it would just look like a bug
  // in the translation.
  it("leaves a placeholder nothing was passed for intact", () => {
    expect(fmt("{missing} here", { other: "x" })).toBe("{missing} here");
    expect(fmt("no vars {x}")).toBe("no vars {x}");
  });
});

describe("rich", () => {
  it("renders the inline tags translations are allowed to carry", () => {
    render(<div data-testid="out">{rich("run <code>git status</code>")}</div>);
    const out = screen.getByTestId("out");
    expect(out.querySelector("code")).toHaveTextContent("git status");
    expect(out).toHaveTextContent("run git status");
  });

  it("nests a tag inside another", () => {
    render(<div data-testid="out">{rich("<b>bold <code>c</code></b>")}</div>);
    const bold = screen.getByTestId("out").querySelector("b");
    expect(bold?.querySelector("code")).toHaveTextContent("c");
  });

  // `<a>` is the one tag whose behaviour comes from the caller: a button when
  // the app handles it, a real link when it leaves the app.
  it("turns <a> into a button that calls onLink", async () => {
    const onLink = vi.fn();
    render(<div>{rich("open <a>settings</a>", { onLink })}</div>);
    screen.getByRole("button", { name: "settings" }).click();
    expect(onLink).toHaveBeenCalledOnce();
  });

  it("turns <a> into an external link when href is given", () => {
    render(
      <div>{rich("see <a>docs</a>", { href: "https://example.com" })}</div>,
    );
    const link = screen.getByRole("link", { name: "docs" });
    expect(link).toHaveAttribute("href", "https://example.com");
    expect(link).toHaveAttribute("rel", "noreferrer");
  });

  it("passes plain text through untouched", () => {
    render(<div data-testid="out">{rich("nothing to parse")}</div>);
    expect(screen.getByTestId("out")).toHaveTextContent("nothing to parse");
  });

  // An unclosed tag is a translation bug; it must not swallow the rest of the
  // string or throw inside a render.
  it("does not lose text after an unclosed tag", () => {
    render(<div data-testid="out">{rich("a <code>b")}</div>);
    expect(screen.getByTestId("out")).toHaveTextContent("a b");
  });
});
