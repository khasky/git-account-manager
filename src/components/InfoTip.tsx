import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** Kept clear of the window edges so a tip near one is not sliced in half. */
const MARGIN = 8;
const MAX_WIDTH = 256;

interface Position {
  left: number;
  top: number;
  width: number;
}

/** An `i` that explains the control next to it.
 *
 *  Opens on hover for a mouse and on click for everything else — a tooltip that
 *  only answers to hover is unreachable by keyboard and by touch, which is the
 *  audience most likely to need the explanation.
 *
 *  Positioned against the viewport rather than the button's own box: the panels
 *  it lives in scroll and clip, so an absolutely placed tip was cut off by its
 *  own container as well as by the window edge. */
export default function InfoTip({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<Position | null>(null);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const tipRef = useRef<HTMLSpanElement>(null);

  // Measured before paint, so the tip never appears at one place and jumps.
  useLayoutEffect(() => {
    if (!open || !buttonRef.current || !tipRef.current) {
      setPos(null);
      return;
    }
    const anchor = buttonRef.current.getBoundingClientRect();
    const width = Math.min(MAX_WIDTH, window.innerWidth - MARGIN * 2);
    const height = tipRef.current.offsetHeight;

    const centred = anchor.left + anchor.width / 2 - width / 2;
    const left = Math.min(
      Math.max(centred, MARGIN),
      window.innerWidth - width - MARGIN,
    );

    // Above by default; below when there is no room, which is the case for a
    // control near the top of the window.
    const above = anchor.top - height - 6;
    const top = above >= MARGIN ? above : anchor.bottom + 6;

    setPos({ left, top, width });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    // Scrolling moves the anchor out from under a viewport-fixed tip.
    function close() {
      setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [open]);

  return (
    <span ref={wrapRef} className="inline-flex align-middle">
      <button
        ref={buttonRef}
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
          ref={tipRef}
          role="tooltip"
          style={{
            left: pos?.left ?? 0,
            top: pos?.top ?? 0,
            width: pos?.width ?? MAX_WIDTH,
            visibility: pos ? "visible" : "hidden",
          }}
          className="fixed z-50 rounded-md border border-bd bg-dialog px-2.5 py-2 text-[11px] leading-relaxed font-normal text-fg-3 shadow-lg"
        >
          {text}
        </span>
      )}
    </span>
  );
}
