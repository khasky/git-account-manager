/** The running half of a button's label. Sized in `em` so it matches whatever
 *  text it sits next to without each caller picking a pixel size. */
export default function Spinner({ className = "" }: { className?: string }) {
  return (
    <svg
      className={`h-[1em] w-[1em] shrink-0 animate-spin ${className}`}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-90"
        fill="currentColor"
        d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z"
      />
    </svg>
  );
}
