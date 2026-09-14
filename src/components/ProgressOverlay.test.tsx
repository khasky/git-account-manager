import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProgressOverlay from "./ProgressOverlay";

function pressEscape() {
  document.body.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
  );
}

describe("ProgressOverlay", () => {
  it("reports how far the action has got", () => {
    render(
      <ProgressOverlay
        open
        title="Saving..."
        label="D:/repos: 11 of 22 repositories"
        percent={25}
      />,
    );
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "25",
    );
    expect(screen.getByText("25%")).toBeInTheDocument();
  });

  it("puts no number on a step the backend has not counted yet", () => {
    render(
      <ProgressOverlay open title="Saving..." label="Saving" percent={null} />,
    );
    expect(screen.getByRole("progressbar")).not.toHaveAttribute(
      "aria-valuenow",
    );
  });

  // The page underneath keeps its own Escape handler; while the action runs it
  // must not hear the key, or the form closes over a save still writing rules.
  it("keeps Escape from reaching the page it covers", () => {
    const onKey = vi.fn();
    window.addEventListener("keydown", onKey);

    const view = render(
      <ProgressOverlay open title="Saving..." label="Saving" percent={null} />,
    );
    pressEscape();
    expect(onKey).not.toHaveBeenCalled();

    view.rerender(
      <ProgressOverlay
        open={false}
        title="Saving..."
        label="Saving"
        percent={null}
      />,
    );
    pressEscape();
    expect(onKey).toHaveBeenCalledTimes(1);

    window.removeEventListener("keydown", onKey);
  });
});
