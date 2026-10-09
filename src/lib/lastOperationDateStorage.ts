import { todayIso } from "@/ui/format/date";

const KEY_PREFIX = "last_operation_date_";
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * The date to pre-fill an operation form with for a given account: the date of the
 * last operation entered on that account, or today when none has been recorded yet.
 */
export function getLastOperationDate(accountId: string): string {
  if (!accountId) return todayIso();
  const stored = localStorage.getItem(`${KEY_PREFIX}${accountId}`);
  return stored && ISO_DATE.test(stored) ? stored : todayIso();
}

/**
 * Remember the date just used for an operation (buy, sell, dividend, deposit,
 * withdrawal, free shares) so the next operation form on the same account pre-fills it.
 */
export function setLastOperationDate(accountId: string, isoDate: string): void {
  if (!accountId || !ISO_DATE.test(isoDate)) return;
  localStorage.setItem(`${KEY_PREFIX}${accountId}`, isoDate);
}
