import { ChevronDown } from "lucide-react";
import type { SelectHTMLAttributes } from "react";

interface SelectFieldProps extends SelectHTMLAttributes<HTMLSelectElement> {
  id: string;
  label: string;
  error?: string;
  options: { label: string; value: string | number }[];
}

export function SelectField({
  id,
  label,
  error,
  options,
  className = "",
  ...props
}: SelectFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="m3-input-label">
        {label}
      </label>
      <div className="relative group">
        <select
          id={id}
          className={`m3-input w-full appearance-none cursor-pointer ${error ? "border-m3-error" : ""} ${className}`}
          {...props}
        >
          {(options ?? []).map((option) => (
            <option
              key={option.value}
              value={option.value}
              className="bg-m3-surface text-m3-on-surface"
            >
              {option.label}
            </option>
          ))}
        </select>
        <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-m3-on-surface-variant group-focus-within:text-m3-primary transition-colors">
          <ChevronDown size={20} />
        </div>
      </div>
      {error && <span className="text-xs text-m3-error px-1">{error}</span>}
    </div>
  );
}
