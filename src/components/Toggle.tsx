interface Props {
  on: boolean;
  onClick: () => void;
  /** `sm` sits inside a dense list row, `md` next to a settings label. */
  size?: "sm" | "md";
}

const SIZES = {
  sm: { track: "h-5 w-9", knob: "h-4 w-4", shift: "translate-x-4" },
  md: { track: "h-6 w-11", knob: "h-5 w-5", shift: "translate-x-5" },
};

export default function Toggle({ on, onClick, size = "md" }: Props) {
  const style = SIZES[size];
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={on}
      className={`relative inline-flex ${style.track} shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
        on ? "bg-emerald-600" : "bg-toggle-off"
      }`}
    >
      <span
        className={`inline-block ${style.knob} rounded-full bg-white shadow transition-transform ${
          on ? style.shift : "translate-x-0"
        }`}
      />
    </button>
  );
}
