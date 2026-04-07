import { useEffect, useRef } from "react";

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

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        const cancel = actions.find((a) => a.variant === "cancel");
        cancel?.onClick();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, actions]);

  if (!open) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      onClick={(e) => {
        if (e.target === overlayRef.current) {
          const cancel = actions.find((a) => a.variant === "cancel");
          cancel?.onClick();
        }
      }}
    >
      <div className="mx-4 w-full max-w-md rounded-lg border border-bd bg-dialog shadow-2xl">
        <div className="border-b border-bd px-5 py-4">
          <h3 className="text-base font-semibold text-fg">{title}</h3>
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
