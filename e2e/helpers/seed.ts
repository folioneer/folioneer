import assert from "node:assert";
import { browser } from "@wdio/globals";

export async function seedCategory(label: string): Promise<string> {
  const cat = (await browser.executeAsync((lbl: string, done: (r: unknown) => void) => {
    // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
    window.__TAURI_INTERNALS__
      .invoke("add_category", { label: lbl })
      .then(done)
      .catch((err: unknown) => done({ __error: String(err) }));
  }, label)) as { id: string };
  assert.ok(!("__error" in cat), `seedCategory failed: ${JSON.stringify(cat)}`);
  return cat.id;
}

export async function seedAccount(
  name: string,
  currency = "EUR",
  updateFrequency = "ManualMonth",
  managementFeesEnabled = false,
): Promise<string> {
  const acc = (await browser.executeAsync(
    (n: string, c: string, freq: string, fees: boolean, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("add_account", {
          dto: {
            name: n,
            currency: c,
            update_frequency: freq,
            management_fees_enabled: fees,
            bank_name: "",
          },
        })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    name,
    currency,
    updateFrequency,
    managementFeesEnabled,
  )) as { id: string };
  assert.ok(!("__error" in acc), `seedAccount failed: ${JSON.stringify(acc)}`);
  return acc.id;
}

export async function seedAsset(
  name: string,
  categoryId: string,
  options?: { reference?: string; assetClass?: string },
): Promise<string> {
  const reference = options?.reference ?? name.slice(0, 6).toUpperCase();
  const assetClass = options?.assetClass ?? "Stocks";
  const asset = (await browser.executeAsync(
    (n: string, ref: string, catId: string, cls: string, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("add_asset", {
          dto: {
            name: n,
            reference: ref,
            class: cls,
            category_id: catId,
            currency: "EUR",
            risk_level: 3,
            interest_bearing: false,
          },
        })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    name,
    reference,
    categoryId,
    assetClass,
  )) as { id: string };
  assert.ok(!("__error" in asset), `seedAsset failed: ${JSON.stringify(asset)}`);
  return asset.id;
}

/**
 * Records a cash deposit on the given account (CSH-022). Used both as a
 * dedicated seed helper for cash flows and internally by `seedBuy` to satisfy
 * the CSH-041 "buy needs cash" guard.
 */
export async function seedDeposit(
  accountId: string,
  date: string,
  amountMicros: number,
): Promise<void> {
  const result = (await browser.executeAsync(
    (accId: string, d: string, amt: number, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("record_deposit", {
          dto: {
            account_id: accId,
            date: d,
            amount_micros: amt,
            note: "",
          },
        })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    accountId,
    date,
    amountMicros,
  )) as { id?: string; __error?: string };
  assert.ok(!("__error" in result), `seedDeposit failed: ${JSON.stringify(result)}`);
}

export async function seedBuy(
  accountId: string,
  assetId: string,
  date: string,
  quantity: number,
): Promise<void> {
  // CSH-041 — buy now needs sufficient cash on the account. Seed a large
  // deposit on the day before the buy so any seedBuy callsite keeps working
  // without forcing every test to thread a deposit through manually.
  const cashSeedDate = new Date(`${date}T00:00:00Z`);
  cashSeedDate.setUTCDate(cashSeedDate.getUTCDate() - 1);
  const depositDate = cashSeedDate.toISOString().slice(0, 10);
  await seedDeposit(accountId, depositDate, 1_000_000_000_000);

  const result = (await browser.executeAsync(
    (accId: string, astId: string, d: string, qty: number, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("buy_holding", {
          dto: {
            account_id: accId,
            asset_id: astId,
            date: d,
            quantity: qty,
            unit_price: 100_000_000,
            exchange_rate: 1_000_000,
            fees: 0,
            total_amount: null,
            note: "",
          },
        })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    accountId,
    assetId,
    date,
    quantity,
  )) as { id?: string; __error?: string };
  assert.ok(!("__error" in result), `seedBuy failed: ${JSON.stringify(result)}`);
}

/**
 * Records a cash dividend via IPC (`record_dividend`). Used so dividend E2E can
 * exercise the happy path without driving the asset ComboboxField, which cannot
 * be automated in WebKitGTK (ADR 007 — docs/adr/007-e2e-combobox-boundary.md).
 *
 * The exchange rate is fixed at 1:1, so this helper only covers dividends whose
 * paying-asset currency equals the account currency. A foreign-currency dividend
 * test would extend it with a rate parameter when that case is actually needed.
 *
 * @param accountId    - account receiving the dividend
 * @param assetId      - the held paying asset (must have quantity > 0, DIV-011)
 * @param date         - ISO 8601 date string (e.g. "2020-06-15")
 * @param amountMicros - net dividend amount in the (same) account currency, i64 micros
 */
export async function seedDividend(
  accountId: string,
  assetId: string,
  date: string,
  amountMicros: number,
): Promise<void> {
  const result = (await browser.executeAsync(
    (accId: string, astId: string, d: string, amount: number, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("record_dividend", {
          dto: {
            account_id: accId,
            asset_id: astId,
            date: d,
            amount_micros: amount,
            exchange_rate: 1_000_000,
            note: "",
          },
        })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    accountId,
    assetId,
    date,
    amountMicros,
  )) as { id?: string; __error?: string };
  assert.ok(!("__error" in result), `seedDividend failed: ${JSON.stringify(result)}`);
}

/**
 * Records a manual asset price via IPC (`record_asset_price`). The written
 * record carries source=Manual (MKT-101).
 *
 * Argument shape is flat (`{ assetId, date, price }`) — the Tauri command
 * takes positional args, not a `dto` envelope. Other seed helpers wrap in
 * `{ dto: {...} }` because their commands take DTO structs.
 *
 * @param assetId  - UUID of the asset
 * @param date     - ISO 8601 date string (e.g. "2020-01-15")
 * @param price    - human-readable decimal (e.g. 42.5); backend converts to i64 micros
 */
export async function seedAssetPrice(assetId: string, date: string, price: number): Promise<void> {
  const result = (await browser.executeAsync(
    (astId: string, d: string, p: number, done: (r: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("record_asset_price", { assetId: astId, date: d, price: p })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    assetId,
    date,
    price,
  )) as { __error?: string } | null;
  assert.ok(
    !(result !== null && typeof result === "object" && "__error" in result),
    `seedAssetPrice failed: ${JSON.stringify(result)}`,
  );
}

/**
 * Records a manual currency rate via IPC (`record_currency_rate`). The written
 * record carries source=Manual (FXR-025).
 *
 * Argument shape is flat (`{ fromCurrency, toCurrency, date, rate }`) — the
 * Tauri command takes positional args, not a `dto` envelope. Other seed helpers
 * wrap in `{ dto: {...} }` because their commands take DTO structs.
 *
 * @param fromCurrency - ISO 4217 source currency (e.g. "USD")
 * @param toCurrency   - ISO 4217 target currency (e.g. "EUR")
 * @param date         - ISO 8601 date string (e.g. "2020-04-10")
 * @param rate         - human-readable decimal (e.g. 1.08); backend converts to micros
 */
export async function seedCurrencyRate(
  fromCurrency: string,
  toCurrency: string,
  date: string,
  rate: number,
): Promise<void> {
  const result = (await browser.executeAsync(
    (from: string, to: string, d: string, r: number, done: (res: unknown) => void) => {
      // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
      window.__TAURI_INTERNALS__
        .invoke("record_currency_rate", { fromCurrency: from, toCurrency: to, date: d, rate: r })
        .then(done)
        .catch((err: unknown) => done({ __error: String(err) }));
    },
    fromCurrency,
    toCurrency,
    date,
    rate,
  )) as { __error?: string } | null;
  assert.ok(
    !(result !== null && typeof result === "object" && "__error" in result),
    `seedCurrencyRate failed: ${JSON.stringify(result)}`,
  );
}
