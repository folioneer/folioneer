import type { RowTone } from "./previewPresenter";

const TONES: Record<RowTone, string> = {
  ready: "bg-m3-primary text-m3-on-primary",
  neutral: "bg-m3-surface-container-highest text-m3-on-surface-variant",
  unknown: "bg-m3-warning-container text-m3-on-warning-container",
  rejected: "bg-m3-error-container text-m3-on-error-container",
};

const CHIP = "inline-flex whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-medium";

/** A count or a status, in its status's colours (IMP-066); with `onClick`, it opens something. */
export function StatusChip({
  tone,
  label,
  id,
  onClick,
}: {
  tone: RowTone;
  label: string;
  id?: string;
  onClick?: () => void;
}) {
  if (onClick) {
    return (
      <button
        id={id}
        type="button"
        className={`${CHIP} ${TONES[tone]} cursor-pointer underline-offset-2 hover:underline`}
        onClick={onClick}
      >
        {label}
      </button>
    );
  }
  return (
    <span id={id} className={`${CHIP} ${TONES[tone]}`}>
      {label}
    </span>
  );
}
