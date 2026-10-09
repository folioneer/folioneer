import type { ReactNode } from "react";

interface CheckboxFieldProps {
  /** F25 — stable id of the checkbox, targetable from E2E. */
  id: string;
  label: ReactNode;
  /** What switching it does, under the label. */
  description?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** A checkbox with its label and, under it, a line saying what it does. */
export function CheckboxField({
  id,
  label,
  description,
  checked,
  onChange,
  disabled = false,
}: CheckboxFieldProps) {
  return (
    <label
      htmlFor={id}
      className={`flex items-start gap-3 group ${disabled ? "opacity-60" : "cursor-pointer"}`}
    >
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        aria-describedby={description ? `${id}-description` : undefined}
        className="accent-m3-primary w-4 h-4 mt-1"
      />
      <span className="flex flex-col gap-1">
        <span
          className={`text-sm font-medium text-m3-on-surface transition-colors ${disabled ? "" : "group-hover:text-m3-primary"}`}
        >
          {label}
        </span>
        {description && (
          <span id={`${id}-description`} className="text-xs text-m3-on-surface-variant">
            {description}
          </span>
        )}
      </span>
    </label>
  );
}
