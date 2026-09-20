import type { TextareaHTMLAttributes } from "react";

interface TextareaFieldProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  id: string;
  label: string;
  error?: string;
}

export function TextareaField({ id, label, error, className = "", ...props }: TextareaFieldProps) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="m3-input-label">
        {label}
      </label>
      <textarea
        id={id}
        className={`m3-input w-full resize-none ${error ? "border-m3-error" : ""} ${className}`}
        {...props}
      />
      {error && <p className="text-xs text-m3-error mt-1 ml-1">{error}</p>}
    </div>
  );
}
