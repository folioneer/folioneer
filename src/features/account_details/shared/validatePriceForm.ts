export function isPriceValid(price: string): boolean {
  if (price.length === 0) return false;
  const n = parseFloat(price);
  return Number.isFinite(n) && n > 0;
}

export function isDateValid(date: string): boolean {
  return (
    date.length > 0 &&
    /^\d{4}-\d{2}-\d{2}$/.test(date) &&
    date <= new Date().toISOString().slice(0, 10)
  );
}
