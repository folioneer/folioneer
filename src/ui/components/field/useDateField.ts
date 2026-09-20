import { useCallback, useEffect, useRef, useState } from "react";

// Calendar popup dimensions for viewport clamping (w-64 = 256px, approx height 290px)
const CALENDAR_WIDTH = 256;
const CALENDAR_HEIGHT = 290;

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const pad2 = (n: number) => String(n).padStart(2, "0");
const toIsoLocal = (d: Date) => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
const todayIsoLocal = () => toIsoLocal(new Date());

/**
 * useDateField - Logic for the DateField component.
 *
 * Manages display/storage date format conversion, calendar visibility,
 * fixed-position calculation to avoid viewport overflow, and day selection.
 */
export function useDateField(
  value: string | number | readonly string[] | undefined,
  onChange: ((e: React.ChangeEvent<HTMLInputElement>) => void) | undefined,
  locale: string,
) {
  const [showCalendar, setShowCalendar] = useState(false);
  const [calendarPos, setCalendarPos] = useState({ top: 0, left: 0 });
  const [displayValue, setDisplayValue] = useState("");
  const [currentMonth, setCurrentMonth] = useState(new Date());
  const inputRef = useRef<HTMLInputElement>(null);
  const calendarRef = useRef<HTMLDivElement>(null);
  // The ISO value this field last emitted via onChange. When the incoming `value`
  // prop equals it, the prop is just echoing our own edit — re-deriving displayValue
  // from it would clobber in-progress typing (a partial date emits "" and would wipe
  // the field). We only re-sync the display on a genuinely external change.
  const lastEmittedIso = useRef<string | undefined>(undefined);

  // Format ISO date (YYYY-MM-DD) to locale string (e.g., DD/MM/YYYY for fr-FR)
  const formatDateForDisplay = useCallback(
    (isoDate: string | number | readonly string[] | undefined): string => {
      if (!isoDate) return "";
      const dateStr = String(isoDate);
      const date = new Date(`${dateStr}T00:00:00Z`);
      if (Number.isNaN(date.getTime())) return dateStr;
      return new Intl.DateTimeFormat(locale).format(date);
    },
    [locale],
  );

  // Parse locale string (e.g., DD/MM/YYYY) to ISO date (YYYY-MM-DD)
  const formatDateForStorage = useCallback(
    (displayDate: string): string => {
      if (!displayDate) return "";
      const localeObj = new Intl.DateTimeFormat(locale);
      const parts = localeObj.formatToParts(new Date());
      const dateParts = displayDate.split(/[/-]/);
      if (dateParts.length !== 3) return "";

      const pattern = parts
        .filter((p) => ["day", "month", "year"].includes(p.type))
        .map((p) => p.type);

      const isoYear = dateParts[pattern.indexOf("year")] ?? "";
      const isoMonth = (dateParts[pattern.indexOf("month")] ?? "").padStart(2, "0");
      const isoDay = (dateParts[pattern.indexOf("day")] ?? "").padStart(2, "0");

      return `${isoYear}-${isoMonth}-${isoDay}`;
    },
    [locale],
  );

  // Sync display value only when the ISO value changes from outside the field
  // (calendar pick routed through the parent, prefill, or a programmatic reset).
  // Skip echoes of our own onChange so partial typing is never wiped.
  useEffect(() => {
    if (value === undefined) return;
    const incoming = String(value);
    if (incoming === (lastEmittedIso.current ?? "")) return;

    lastEmittedIso.current = incoming;
    setDisplayValue(formatDateForDisplay(value));
    if (incoming.match(/^\d{4}-\d{2}-\d{2}$/)) {
      const [year, month] = incoming.split("-") as [string, string];
      setCurrentMonth(new Date(parseInt(year, 10), parseInt(month, 10) - 1, 1));
    }
  }, [value, formatDateForDisplay]);

  // Open calendar and compute fixed position based on input bounding rect
  const openCalendar = () => {
    if (inputRef.current) {
      const rect = inputRef.current.getBoundingClientRect();
      const gap = 4;

      let top = rect.bottom + gap;
      let left = rect.left;

      if (left + CALENDAR_WIDTH > window.innerWidth - 8) {
        left = window.innerWidth - CALENDAR_WIDTH - 8;
      }
      if (top + CALENDAR_HEIGHT > window.innerHeight - 8) {
        top = rect.top - CALENDAR_HEIGHT - gap;
      }

      setCalendarPos({ top, left });
    }
    setShowCalendar(true);
  };

  const closeCalendar = () => {
    // Revert the display to the last committed value when focus leaves: a half-typed
    // entry (which emits "" and leaves stale partial text) — or a stale display after
    // an external reset to "" that the echo-skip guard can't distinguish — is resolved
    // to the authoritative value instead of lingering.
    setDisplayValue(formatDateForDisplay(lastEmittedIso.current));
    setTimeout(() => setShowCalendar(false), 200);
  };

  const clearDate = () => {
    setDisplayValue("");
    setShowCalendar(false);
    lastEmittedIso.current = "";
    onChange?.({
      target: { value: "" },
    } as React.ChangeEvent<HTMLInputElement>);
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newDisplayValue = e.target.value;
    setDisplayValue(newDisplayValue);

    const isoDate = formatDateForStorage(newDisplayValue);
    const validIso = isoDate && /^\d{4}-\d{2}-\d{2}$/.test(isoDate) ? isoDate : "";
    lastEmittedIso.current = validIso;
    onChange?.({
      ...e,
      target: { ...e.target, value: validIso },
    } as React.ChangeEvent<HTMLInputElement>);
  };

  // Step the committed date by `deltaDays`; an empty field jumps straight to today.
  const stepDate = (deltaDays: number) => {
    const current = lastEmittedIso.current;
    let isoDate: string;
    if (current && ISO_DATE.test(current)) {
      const [year, month, day] = current.split("-").map(Number) as [number, number, number];
      const next = new Date(year, month - 1, day);
      next.setDate(next.getDate() + deltaDays);
      isoDate = toIsoLocal(next);
    } else {
      isoDate = todayIsoLocal();
    }
    setDisplayValue(formatDateForDisplay(isoDate));
    lastEmittedIso.current = isoDate;
    const [year, month] = isoDate.split("-").map(Number) as [number, number, number];
    setCurrentMonth(new Date(year, month - 1, 1));
    onChange?.({
      target: { value: isoDate },
    } as React.ChangeEvent<HTMLInputElement>);
  };

  // +/= increments and - decrements the date by one day. Only fires on a settled
  // value (empty or a complete date) so a "-" separator typed mid-entry isn't hijacked.
  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "+" && e.key !== "=" && e.key !== "-") return;
    const settled = displayValue === "" || ISO_DATE.test(formatDateForStorage(displayValue));
    if (!settled) return;
    e.preventDefault();
    stepDate(e.key === "-" ? -1 : 1);
  };

  const handleDateSelect = (date: Date) => {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    const isoDate = `${year}-${month}-${day}`;
    setDisplayValue(formatDateForDisplay(isoDate));
    setShowCalendar(false);
    lastEmittedIso.current = isoDate;
    onChange?.({
      target: { value: isoDate },
    } as React.ChangeEvent<HTMLInputElement>);
    inputRef.current?.blur();
  };

  const getDaysInMonth = (date: Date) =>
    new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate();

  const getFirstDayOfMonth = (date: Date) =>
    new Date(date.getFullYear(), date.getMonth(), 1).getDay();

  const monthYear = new Intl.DateTimeFormat(locale, {
    month: "long",
    year: "numeric",
  }).format(currentMonth);

  return {
    displayValue,
    showCalendar,
    calendarPos,
    currentMonth,
    setCurrentMonth,
    inputRef,
    calendarRef,
    monthYear,
    openCalendar,
    closeCalendar,
    clearDate,
    handleInputChange,
    handleKeyDown,
    handleDateSelect,
    getDaysInMonth,
    getFirstDayOfMonth,
  };
}
