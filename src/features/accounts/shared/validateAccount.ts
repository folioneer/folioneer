import type { I18nMessage } from "@/ui/format/i18n";

// R14 — block submission if name is empty or whitespace-only
export function validateAccountName(name: string): I18nMessage | null {
  if (name.trim().length === 0) {
    return { key: "account.error_name_required" };
  }
  return null;
}

// Block submission if currency is not a 3-letter uppercase ISO 4217 code
export function validateAccountCurrency(currency: string): I18nMessage | null {
  if (!/^[A-Z]{3}$/.test(currency.trim())) {
    return { key: "account.error_currency_invalid" };
  }
  return null;
}
