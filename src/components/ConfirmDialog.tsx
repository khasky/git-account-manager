import { useEffect, useId, useRef } from "react";

export interface DialogAction {
  label: string;
  variant: "danger" | "default" | "cancel";
  onClick: () => void;
}

interface Props {
  open: boolean;
  title: string;
  children: React.ReactNode;
  actions: DialogAction[];
}

export default function ConfirmDialog({
  open,
  title,
  children,
  actions,
}: Props) {
  const overlayRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  // Read through a ref so the key handler is registered once per open rather
  // than on every render: `actions` is a fresh array each time.
  const cancelRef = useRef<(() => void) | undefined>(undefined);
  cancelRef.current = actions.find((a) => a.variant === "cancel")?.onClick;

  useEffect(() => {
    if (!open) return;
    const focusable = () =>
      Array.from(
        panelRef.current?.querySelectorAll<HTMLElement>(
          "button:not([disabled])",
        ) ?? [],
      );
    // The panel takes focus, not the first button: that button is usually the
    // destructive one and Enter would fire it.
    panelRef.current?.focus();

    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        cancelRef.current?.();
        return;
      }
      if (e.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  if (!open) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      onClick={(e) => {
        if (e.target === overlayRef.current) cancelRef.current?.();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className="mx-4 w-full max-w-md rounded-lg border border-bd bg-dialog shadow-2xl outline-none"
      >
        <div className="border-b border-bd px-5 py-4">
          <h3 id={titleId} className="text-base font-semibold text-fg">
            {title}
          </h3>
        </div>
        <div className="px-5 py-4">{children}</div>
        <div className="flex flex-col gap-2 border-t border-bd px-5 py-4">
          {actions.map((action, i) => {
            const base =
              "w-full rounded-md px-4 py-2 text-sm font-medium transition-colors";
            const style =
              action.variant === "danger"
                ? `${base} bg-red-600 text-white hover:bg-red-500`
                : action.variant === "cancel"
                  ? `${base} bg-subtle text-fg-3 hover:bg-hover`
                  : `${base} bg-blue-600 text-white hover:bg-blue-500`;
            return (
              <button key={i} onClick={action.onClick} className={style}>
                {action.label}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
