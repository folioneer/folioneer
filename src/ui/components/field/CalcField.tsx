import { useEffect, useRef, useState } from "react";
import { evaluateArithmetic } from "./arithmetic";

interface CalcFieldProps {
  id: string;
  label: string;
  /** Committed value (a plain number string) owned by the form. */
  value: string;
  /** Receives the evaluated value (plain numbers pass through untouched). */
  onValueChange: (value: string) => void;
  error?: string;
  placeholder?: string;
  required?: boolean;
  "data-testid"?: string;
}

/** True when the text holds an arithmetic operator (not just a leading sign). */
function hasArithmetic(raw: string): boolean {
  const compact = raw.replace(/\s/g, "");
  return /[+*/()]/.test(compact) || /\d-/.test(compact);
}

/**
 * Normalises a result to ≤6 decimals (micro precision) with trailing zeros
 * trimmed, so float artifacts like `50 * 0.1 = 5.000000000000001` render and
 * commit as "5" (dot decimal).
 */
function formatResult(n: number): string {
  return String(Number(n.toFixed(6)));
}

/** The value to report up: the formatted result for expressions, raw otherwise. */
function reportedValue(raw: string): string {
  if (!hasArithmetic(raw)) return raw;
  const result = evaluateArithmetic(raw);
  return result !== null ? formatResult(result) : raw;
}

/**
 * A number field that also accepts inline arithmetic (`+ - * / ( )`). While the
 * user types an expression a `= result` hint appears; on blur the expression is
 * replaced with its result. The form always receives the evaluated numeric
 * value via `onValueChange` (A3 — inline calc). Plain numbers behave exactly
 * like the `type="number"` field it replaces.
 */
export function CalcField({
  id,
  label,
  value,
  onValueChange,
  error,
  placeholder,
  required,
  "data-testid": dataTestId,
}: CalcFieldProps) {
  const [display, setDisplay] = useState(value);
  // Tracks what we last reported up, so an external value change (form reset,
  // pre-fill) re-syncs the display while our own reports do not clobber it.
  const lastReported = useRef(value);

  useEffect(() => {
    if (value !== lastReported.current) {
      setDisplay(value);
      lastReported.current = value;
    }
  }, [value]);

  const previewResult = hasArithmetic(display) ? evaluateArithmetic(display) : null;

  const handleInput = (raw: string) => {
    setDisplay(raw);
    const reported = reportedValue(raw);
    lastReported.current = reported;
    onValueChange(reported);
  };

  const handleBlur = () => {
    if (hasArithmetic(display)) {
      const result = evaluateArithmetic(display);
      if (result !== null) setDisplay(formatResult(result));
    }
  };

  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="m3-input-label">
        {label}
      </label>
      <input
        id={id}
        data-testid={dataTestId}
        type="text"
        inputMode="decimal"
        autoComplete="off"
        className={`m3-input w-full ${error ? "border-m3-error" : ""}`}
        value={display}
        onChange={(e) => handleInput(e.target.value)}
        onBlur={handleBlur}
        placeholder={placeholder}
        required={required}
      />
      {previewResult !== null && (
        <p className="text-xs text-m3-on-surface-variant mt-1 ml-1" aria-live="polite">
          = {formatResult(previewResult)}
        </p>
      )}
      {error && <p className="text-xs text-m3-error mt-1 ml-1">{error}</p>}
    </div>
  );
}
