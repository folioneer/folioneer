import { Combobox, ComboboxInput, ComboboxOption, ComboboxOptions } from "@headlessui/react";
import { ChevronDown } from "lucide-react";
import { useMemo } from "react";
import { useComboboxField } from "./useComboboxField";

interface ComboboxFieldProps<T extends object> {
  id: string;
  label: string;
  items: T[];
  displayKey: keyof T;
  idKey: keyof T;
  value: string;
  onChange: (id: string) => void;
  placeholder?: string;
  searchKeys?: (keyof T)[];
  error?: string;
  disabled?: boolean;
  onCreateNew?: (query: string) => void;
  /**
   * Label for the inline "create new" entry — caller-supplied (typically `t(...)`)
   * so no untranslated string ships. The entry renders only when BOTH `onCreateNew`
   * and `createLabel` are provided; there is intentionally no default (F16).
   */
  createLabel?: string;
}

const CREATE_MARKER = "@@CREATE";

/**
 * ComboboxField - M3 Design System Autocomplete + Dropdown field.
 *
 * Generic combobox with fuzzy search, item selection and optional inline
 * "create new" entry. Styling follows the same m3-input / m3-input-label
 * conventions as TextField and SelectField.
 *
 * @example
 * <ComboboxField
 *   id="patient"
 *   label="Patient *"
 *   items={patients}
 *   displayKey="name"
 *   idKey="id"
 *   value={patientId}
 *   onChange={setPatientId}
 *   onCreateNew={(q) => openCreatePatient(q)}
 *   createLabel={t("patient.create_new")}
 * />
 */
export function ComboboxField<T extends object>({
  id,
  label,
  items,
  displayKey,
  idKey,
  value,
  onChange,
  searchKeys,
  placeholder,
  error,
  disabled,
  onCreateNew,
  createLabel,
}: ComboboxFieldProps<T>) {
  const { query, setQuery, filteredItems } = useComboboxField(items, displayKey, searchKeys);

  const selectedItem = useMemo(
    () => items.find((item) => String(item[idKey]) === value) ?? null,
    [items, idKey, value],
  );

  // HeadlessUI owns the open/close state (opened on focus via `immediate`); this
  // flag only avoids rendering an empty options panel when there's nothing to show.
  const showDropdown = filteredItems.length > 0 || !!(onCreateNew && createLabel);

  const handleChange = (selected: T | typeof CREATE_MARKER | null) => {
    if (!selected) return;
    if (selected === CREATE_MARKER) {
      onCreateNew?.(query);
    } else {
      onChange(String((selected as T)[idKey]));
      setQuery("");
    }
  };

  const inputClassName = [
    "m3-input w-full pr-10",
    error ? "border-m3-error" : "",
    disabled ? "opacity-50 cursor-not-allowed" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={id} className="m3-input-label">
        {label}
      </label>

      <Combobox immediate disabled={disabled} value={selectedItem} onChange={handleChange}>
        <div className="relative group">
          <ComboboxInput
            id={id}
            className={inputClassName}
            displayValue={(item: T | null) => query || (item ? String(item[displayKey]) : "")}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={placeholder}
            autoComplete="off"
          />
          <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-m3-on-surface-variant group-focus-within:text-m3-primary transition-colors">
            <ChevronDown size={20} />
          </div>

          {showDropdown && (
            <ComboboxOptions
              anchor={{ to: "bottom start", gap: "4px" }}
              className="z-50 w-(--input-width) bg-m3-surface border border-m3-outline/20 shadow-elevation-3 rounded-lg py-1 overflow-auto max-h-48"
            >
              {filteredItems.slice(0, 6).map((item) => (
                <ComboboxOption
                  key={String(item[idKey])}
                  value={item}
                  className="px-4 py-2 text-sm text-m3-on-surface cursor-pointer data-focus:bg-m3-primary/10 data-focus:text-m3-primary"
                >
                  {String(item[displayKey])}
                </ComboboxOption>
              ))}

              {onCreateNew && createLabel && (
                <ComboboxOption
                  value={CREATE_MARKER}
                  className="px-4 py-2 text-sm font-medium text-m3-primary cursor-pointer border-t border-m3-outline/10 data-focus:bg-m3-primary/10"
                >
                  {createLabel}
                </ComboboxOption>
              )}
            </ComboboxOptions>
          )}
        </div>
      </Combobox>

      {error && <p className="text-xs text-m3-error mt-1 ml-1">{error}</p>}
    </div>
  );
}
