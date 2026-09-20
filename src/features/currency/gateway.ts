import {
  type CurrencyError,
  type CurrencyPair,
  type CurrencyPairSummary,
  type CurrencyRate,
  commands,
  type Event,
  events,
  type RateHistoryBackfillError,
  type Result,
} from "../../bindings";

/**
 * Gateway for Currency-rate backend communication.
 * Centralizes all Tauri command calls for the Currency feature (the only
 * file in the feature allowed to touch `commands.*`). Each function is a
 * typed `Result` pass-through (F27) matching the `bindings.ts` signature.
 */
export async function declareCurrencyPair(
  fromCurrency: string,
  toCurrency: string,
): Promise<Result<CurrencyPair, CurrencyError>> {
  return await commands.declareCurrencyPair(fromCurrency, toCurrency);
}

export async function recordCurrencyRate(
  fromCurrency: string,
  toCurrency: string,
  date: string,
  rate: number,
): Promise<Result<CurrencyRate, CurrencyError>> {
  return await commands.recordCurrencyRate(fromCurrency, toCurrency, date, rate);
}

export async function updateCurrencyRate(
  fromCurrency: string,
  toCurrency: string,
  originalDate: string,
  newDate: string,
  newRate: number,
): Promise<Result<null, CurrencyError>> {
  return await commands.updateCurrencyRate(
    fromCurrency,
    toCurrency,
    originalDate,
    newDate,
    newRate,
  );
}

export async function deleteCurrencyRate(
  fromCurrency: string,
  toCurrency: string,
  date: string,
): Promise<Result<null, CurrencyError>> {
  return await commands.deleteCurrencyRate(fromCurrency, toCurrency, date);
}

export async function getCurrencyPairs(): Promise<Result<CurrencyPairSummary[], CurrencyError>> {
  return await commands.getCurrencyPairs();
}

export async function getCurrencyRates(
  fromCurrency: string,
  toCurrency: string,
): Promise<Result<CurrencyRate[], CurrencyError>> {
  return await commands.getCurrencyRates(fromCurrency, toCurrency);
}

/** FXR-110 — downloads the full dated rate history for every persisted pair. */
export async function backfillCurrencyRateHistory(): Promise<
  Result<number, RateHistoryBackfillError>
> {
  return await commands.backfillCurrencyRateHistory();
}

/** Subscribe to the backend event bus; invokes `callback` with each event's discriminant (FXR-026/037). */
export async function subscribeToEvents(
  callback: (type: Event["type"]) => void,
): Promise<() => void> {
  return events.event.listen((event) => {
    callback(event.payload.type);
  });
}

export const currencyGateway = {
  declareCurrencyPair,
  recordCurrencyRate,
  updateCurrencyRate,
  deleteCurrencyRate,
  getCurrencyPairs,
  getCurrencyRates,
  backfillCurrencyRateHistory,
  subscribeToEvents,
};
