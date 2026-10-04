import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { decimalToDisplayed, typedToDecimal } from "@/lib/microUnits";
import { evaluateArithmetic } from "./arithmetic";

interface CalcFieldProps {
  id: string;
  label: string;
  /** Committed value owned by the form: a plain number written with a dot. */
  value: string;
  /** Receives the evaluated value, written with a dot whatever the display language. */
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
 * value via `onValueChange` (A3 — inline calc). The field shows and accepts the decimal
 * separator of the display language (NUM-010/011): the form's value stays written with a
 * dot.
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
  // The display language decides the separator shown; a change of language re-renders.
  const { i18n } = useTranslation();
  const language = i18n.language;
  const [display, setDisplay] = useState(() => decimalToDisplayed(value));
  // Tracks what we last reported up, so an external value change (form reset,
  // pre-fill) re-syncs the display while our own reports do not clobber it.
  const lastReported = useRef(value);
  const lastLanguage = useRef(language);

  useEffect(() => {
    if (value !== lastReported.current || language !== lastLanguage.current) {
      setDisplay(decimalToDisplayed(value));
      lastReported.current = value;
      lastLanguage.current = language;
    }
  }, [value, language]);

  // A typed dot shown as a comma rewrites the text: the caret is put back where it was.
  const inputRef = useRef<HTMLInputElement>(null);
  const caret = useRef<number | null>(null);
  useLayoutEffect(() => {
    if (caret.current !== null) {
      inputRef.current?.setSelectionRange(caret.current, caret.current);
      caret.current = null;
    }
  });

  const typed = typedToDecimal(display);
  const previewResult = hasArithmetic(typed) ? evaluateArithmetic(typed) : null;

  const handleInput = (raw: string, caretAt: number | null) => {
    const decimal = typedToDecimal(raw);
    const displayed = decimalToDisplayed(decimal);
    if (displayed !== raw) caret.current = caretAt;
    setDisplay(displayed);
    const reported = reportedValue(decimal);
    lastReported.current = reported;
    onValueChange(reported);
  };

  const handleBlur = () => {
    if (previewResult !== null) setDisplay(decimalToDisplayed(formatResult(previewResult)));
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
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${id}-error` : undefined}
        value={display}
        ref={inputRef}
        onChange={(e) => handleInput(e.target.value, e.target.selectionStart)}
        onBlur={handleBlur}
        placeholder={placeholder}
        required={required}
      />
      {previewResult !== null && (
        <p className="text-xs text-m3-on-surface-variant mt-1 ml-1" aria-live="polite">
          = {decimalToDisplayed(formatResult(previewResult))}
        </p>
      )}
      {error && (
        <p id={`${id}-error`} className="text-xs text-m3-error mt-1 ml-1">
          {error}
        </p>
      )}
    </div>
  );
}
