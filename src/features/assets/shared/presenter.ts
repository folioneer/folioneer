import type {
  ArchiveAssetError,
  Asset,
  AssetClass,
  AssetError,
  AssetKind,
  DeleteAssetError,
  OtherListing,
} from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * F27 — Maps any asset-BC mutation error (add / update / archive / unarchive / delete)
 * to an i18n key + interpolation vars. Pure function, no React, no useTranslation.
 *
 * Handles the codes reachable for these commands; the wrapped `AccountError` is a
 * BC-wide union, so any unreachable variant falls through to `error.Unknown`. A refusal
 * for an asset that already exists (AST-032) names it with what tells it apart, read from
 * `assets` when the existing asset is among them.
 */
export function assetMutationErrorToI18n(
  err: AssetError | ArchiveAssetError | DeleteAssetError,
  assets: readonly Asset[] = [],
): I18nMessage {
  switch (err.code) {
    case "AssetAlreadyExists": {
      const existing = assets.find((asset) => asset.id === err.existing_id);
      return {
        key: "error.AssetAlreadyExists",
        vars: {
          existing_name: existing
            ? `${existing.name} (${describeListing(existing)})`
            : err.existing_name,
        },
      };
    }
    case "IsinRequired":
    case "IsinNotAllowed":
    case "ExchangeNotAllowed":
    case "ClassNotAllowed":
      return { key: `error.${err.code}` };
    case "InvalidExchange":
      return { key: "error.InvalidExchange", vars: { exchange_code: err.exchange_code } };
    case "InvalidCurrency":
      return { key: "error.InvalidCurrency", vars: { currency: err.currency } };
    case "NameEmpty":
    case "ReferenceEmpty":
    case "InvalidIsinFormat":
    case "InvalidRiskLevel":
    case "Archived":
    case "CashAssetNotEditable":
    case "AssetNotFound":
    case "CategoryNotFound":
    case "DatabaseError":
    case "AccountNotFound":
    case "NameAlreadyExists":
    case "ActiveHoldings":
    case "ExistingTransactions":
    case "DuplicateName":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/** Returns Tailwind classes for the risk badge — R11 (5 distinct colours). */
export function getRiskBadgeClasses(riskLevel: number): string {
  switch (riskLevel) {
    case 1:
      return "bg-green-100 text-green-700";
    case 2:
      return "bg-green-200 text-green-800";
    case 3:
      return "bg-orange-100 text-orange-700";
    case 4:
      return "bg-red-100 text-red-700";
    case 5:
      return "bg-red-200 text-red-800";
    default:
      return "bg-gray-100 text-gray-600";
  }
}

/** Returns a localised label for an asset class — WEB-031.
 *  Exhaustive switch ensures new variants are caught at compile time. */
export function formatAssetClass(assetClass: AssetClass, t: (key: string) => string): string {
  switch (assetClass) {
    case "Cash":
      return t("asset.class.Cash");
    case "Bonds":
      return t("asset.class.Bonds");
    case "RealEstate":
      return t("asset.class.RealEstate");
    case "MutualFunds":
      return t("asset.class.MutualFunds");
    case "ETF":
      return t("asset.class.ETF");
    case "ETP":
      return t("asset.class.ETP");
    case "Stocks":
      return t("asset.class.Stocks");
    case "DigitalAsset":
      return t("asset.class.DigitalAsset");
    case "Derivatives":
      return t("asset.class.Derivatives");
  }
}

/** What tells one asset from another with the same name: its reference, exchange and currency. */
export function describeListing(
  listing: Pick<Asset, "reference" | "exchange" | "currency"> | OtherListing,
): string {
  const exchange = listing.exchange ? ` · ${listing.exchange.label}` : "";
  return `${listing.reference}${exchange} · ${listing.currency}`;
}

/**
 * AST-039 — the other listings of an asset's instrument, as the table shows them: one in
 * full; several as the first followed by an ellipsis, the whole list being the hint.
 */
export function presentOtherListings(
  others: readonly OtherListing[],
): { shown: string; hint: string } | null {
  const [first, ...rest] = others.map(describeListing);
  if (first === undefined) return null;
  return {
    shown: rest.length === 0 ? first : `${first}, …`,
    hint: [first, ...rest].join("\n"),
  };
}

/** The i18n key of a kind's name — exhaustive, so a new kind is caught at compile time. */
export function assetKindLabelKey(kind: AssetKind): string {
  switch (kind) {
    case "Listed":
      return "asset.kind.Listed";
    case "Crypto":
      return "asset.kind.Crypto";
    case "Custom":
      return "asset.kind.Custom";
    case "Cash":
      return "asset.kind.Cash";
  }
}

/** The i18n key of the line saying what a kind means. */
export function assetKindSaysKey(kind: AssetKind): string {
  switch (kind) {
    case "Listed":
      return "asset.kind_says.Listed";
    case "Crypto":
      return "asset.kind_says.Crypto";
    case "Custom":
      return "asset.kind_says.Custom";
    case "Cash":
      return "asset.kind_says.Cash";
  }
}

/** The i18n key of the reference field's label: what the reference is for that kind. */
export function referenceLabelKey(kind: AssetKind): string {
  switch (kind) {
    case "Listed":
      return "asset.form_reference_label_listed";
    case "Crypto":
      return "asset.form_reference_label_crypto";
    case "Custom":
    case "Cash":
      return "asset.form_reference_label";
  }
}

/** The i18n key of the currency field's label: a crypto asset is quoted in a currency. */
export function currencyLabelKey(kind: AssetKind): string {
  return kind === "Crypto" ? "asset.form_currency_label_crypto" : "asset.form_currency_label";
}
