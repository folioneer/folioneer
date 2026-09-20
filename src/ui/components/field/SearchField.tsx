import { Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";

interface SearchFieldProps {
  id: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

/**
 * SearchField - M3 Design System Search Input Component
 *
 * Search input with integrated clear button and search icon.
 * Layout (max-width) is controlled by parent component.
 *
 * @example
 * <SearchField
 *   id="patient-search"
 *   value={searchTerm}
 *   onChange={setSearchTerm}
 *   placeholder="Search patients..."
 * />
 */
export function SearchField({ id, value, onChange, placeholder = "Search..." }: SearchFieldProps) {
  const { t } = useTranslation("common");
  return (
    <div className="relative flex-1 group">
      <Search className="m3-search-icon" size={18} />
      <input
        id={id}
        type="text"
        placeholder={placeholder}
        className="m3-search-input pr-10"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          className="m3-search-clear-btn"
          aria-label={t("action.clearSearch")}
        >
          <X size={16} strokeWidth={2.5} />
        </button>
      )}
    </div>
  );
}
