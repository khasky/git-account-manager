import { useEffect, useRef, useState } from "react";

/** An `i` that explains the control next to it.
 *
 *  Opens on hover for a mouse and on click for everything else — a tooltip that
 *  only answers to hover is unreachable by keyboard and by touch, which is the
 *  audience most likely to need the explanation. */
export default function InfoTip({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <span ref={ref} className="relative inline-flex align-middle">
      <button
        type="button"
        aria-label={text}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        className="flex h-4 w-4 items-center justify-center rounded-full border border-bd-s text-[10px] leading-none font-semibold text-fg-5 transition-colors hover:border-bd hover:text-fg-3"
      >
        i
      </button>
      {open && (
        <span
          role="tooltip"
          className="absolute bottom-full left-1/2 z-50 mb-1.5 w-64 -translate-x-1/2 rounded-md border border-bd bg-dialog px-2.5 py-2 text-[11px] leading-relaxed font-normal text-fg-3 shadow-lg"
        >
          {text}
        </span>
      )}
    </span>
  );
}
