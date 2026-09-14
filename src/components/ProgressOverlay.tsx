import { useEffect, useId, useRef } from "react";
import Spinner from "./Spinner";

interface Props {
  open: boolean;
  title: string;
  label: string;
  /** Null until the backend reports its first step, when there is nothing to
   *  put a number on yet and the bar only says that work is running. */
  percent: number | null;
}

/**
 * The window while a long action runs: nothing behind it can be clicked, and
 * the bar says how much of the action is left.
 *
 * It sits above every other layer, the folder alert included — a dialog that
 * arrives on a timer must not land on top of an action already under way.
 */
export default function ProgressOverlay({
  open,
  title,
  label,
  percent,
}: Props) {
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useEffect(() => {
    if (!open) return;
    // Focus leaves whatever was behind, so Enter and Space cannot reach a
    // control the overlay is covering.
    panelRef.current?.focus();

    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" || e.key === "Tab") {
        e.preventDefault();
        e.stopPropagation();
        panelRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[110] flex items-center justify-center bg-overlay">
      <div
        ref={panelRef}
        role="alertdialog"
        aria-modal="true"
        aria-busy="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className="mx-4 w-full max-w-md rounded-lg border border-bd bg-dialog p-5 shadow-2xl outline-none"
      >
        <div className="flex items-center gap-2 text-base font-semibold text-fg">
          <Spinner />
          <h3 id={titleId}>{title}</h3>
        </div>

        <div className="mt-4 h-2 overflow-hidden rounded-full bg-subtle">
          <div
            role="progressbar"
            aria-labelledby={titleId}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent ?? undefined}
            className={
              percent === null
                ? "h-full w-full animate-pulse bg-blue-600"
                : "h-full bg-blue-600 transition-[width] duration-200"
            }
            style={percent === null ? undefined : { width: `${percent}%` }}
          />
        </div>

        <div className="mt-2 flex items-baseline justify-between gap-3">
          <p className="min-w-0 truncate text-sm text-fg-3">{label}</p>
          {percent !== null && (
            <span className="shrink-0 font-mono text-sm text-fg-4">
              {percent}%
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
