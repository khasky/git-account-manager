import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useI18n, fmt } from "../i18n";

type State =
  | { kind: "hidden" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; percent: number }
  | { kind: "error"; message: string };

// Checks GitHub Releases for a newer signed build on startup and, when found,
// shows a banner that downloads, installs, and relaunches in one click.
export default function UpdateBanner() {
  const { m } = useI18n();
  const [state, setState] = useState<State>({ kind: "hidden" });

  useEffect(() => {
    let active = true;
    check()
      .then((update) => {
        if (active && update) setState({ kind: "available", update });
      })
      .catch((e) => {
        // Dev builds have no updater and offline machines can't reach the
        // endpoint — neither case is worth surfacing to the user.
        console.error("Update check failed:", e);
      });
    return () => {
      active = false;
    };
  }, []);

  async function runUpdate(update: Update) {
    try {
      let total = 0;
      let downloaded = 0;
      setState({ kind: "downloading", percent: 0 });
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            setState({
              kind: "downloading",
              percent: total ? Math.round((downloaded / total) * 100) : 0,
            });
            break;
          case "Finished":
            setState({ kind: "downloading", percent: 100 });
            break;
        }
      });
      await relaunch();
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }

  if (state.kind === "hidden") return null;

  return (
    <div className="flex items-center justify-between gap-4 border-b border-bd bg-blue-600/10 px-6 py-2.5 text-sm">
      {state.kind === "available" && (
        <>
          <span className="text-fg-2">
            <span className="font-medium text-fg">{m.update.available}</span>
            {" — "}
            {fmt(m.update.newVersion, {
              version: state.update.version,
              current: state.update.currentVersion,
            })}
          </span>
          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={() => runUpdate(state.update)}
              className="rounded-md bg-blue-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
            >
              {m.update.updateNow}
            </button>
            <button
              onClick={() => setState({ kind: "hidden" })}
              className="rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-3 transition-colors hover:bg-subtle hover:text-fg-2"
            >
              {m.update.later}
            </button>
          </div>
        </>
      )}

      {state.kind === "downloading" && (
        <span className="text-fg-2">
          {fmt(m.update.downloading, { percent: state.percent })}
        </span>
      )}

      {state.kind === "error" && (
        <>
          <span className="text-red-400">
            {fmt(m.update.failed, { error: state.message })}
          </span>
          <button
            onClick={() => setState({ kind: "hidden" })}
            className="shrink-0 rounded-md bg-raised px-3 py-1.5 text-xs font-medium text-fg-3 transition-colors hover:bg-subtle hover:text-fg-2"
          >
            {m.update.later}
          </button>
        </>
      )}
    </div>
  );
}
