import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AccountError,
  ArchiveAssetTask,
  Asset,
  AssetError,
  AssetLookupResult,
  CreateAssetDTO,
  DeleteAssetTask,
  UpdateAssetDTO,
  WebLookupError,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { assetGateway } = await import("./gateway");

const makeAsset = (): Asset => ({
  id: "asset-1",
  name: "Apple Inc.",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  category: { id: "cat-1", name: "Equities" },
  currency: "USD",
  risk_level: 3,
  is_archived: false,
  price_refresh_blocked: false,
  interest_bearing: false,
  exchange: null,
});

const baseCreateDto: CreateAssetDTO = {
  name: "Apple Inc.",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 3,
  category_id: "cat-1",
  exchange: null,
  interest_bearing: false,
};

const baseUpdateDto: UpdateAssetDTO = {
  asset_id: "asset-1",
  name: "Apple Inc.",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 3,
  category_id: "cat-1",
  exchange: null,
  interest_bearing: false,
};

describe("asset gateway — CRUD", () => {
  beforeEach(() => vi.clearAllMocks());

  // ── getAssets / getAssetsWithArchived ──────────────────────────────────────

  it("getAssets returns list on success", async () => {
    const assets = [makeAsset()];
    mockInvoke.mockResolvedValue(assets);
    const result = await assetGateway.getAssets();
    expect(result).toEqual({ status: "ok", data: assets });
    expect(mockInvoke).toHaveBeenCalledWith("get_assets");
  });

  it("getAssets surfaces DatabaseError on repo failure", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.getAssets();
    expect(result).toEqual({ status: "error", error: err });
  });

  it("getAssetsWithArchived surfaces DatabaseError on repo failure", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.getAssetsWithArchived();
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── createAsset ────────────────────────────────────────────────────────────

  it("createAsset returns Asset on success", async () => {
    const asset = makeAsset();
    mockInvoke.mockResolvedValue(asset);
    const result = await assetGateway.createAsset(baseCreateDto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("add_asset", { dto: baseCreateDto });
  });

  it("createAsset surfaces NameEmpty domain leaf", async () => {
    const err: AssetError = { code: "NameEmpty" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({ ...baseCreateDto, name: "" });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("createAsset surfaces ReferenceEmpty domain leaf", async () => {
    const err: AssetError = { code: "ReferenceEmpty" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({ ...baseCreateDto, reference: "" });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("createAsset surfaces InvalidCurrency with currency payload", async () => {
    const err: AssetError = { code: "InvalidCurrency", currency: "XYZ" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({ ...baseCreateDto, currency: "XYZ" });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("createAsset surfaces InvalidRiskLevel with received payload", async () => {
    const err: AssetError = { code: "InvalidRiskLevel", received: 9 };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({ ...baseCreateDto, risk_level: 9 });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("createAsset surfaces CategoryNotFound from cross-aggregate category lookup", async () => {
    const err: AssetError = { code: "CategoryNotFound", id: "missing-cat" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({
      ...baseCreateDto,
      category_id: "missing-cat",
      exchange: null,
    });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("createAsset surfaces DatabaseError on repo write failure", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset(baseCreateDto);
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── updateAsset ────────────────────────────────────────────────────────────

  it("updateAsset returns Asset on success", async () => {
    const asset = makeAsset();
    mockInvoke.mockResolvedValue(asset);
    const result = await assetGateway.updateAsset(baseUpdateDto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("update_asset", { dto: baseUpdateDto });
  });

  it("updateAsset surfaces NotFound with asset id payload", async () => {
    const err: AssetError = { code: "AssetNotFound", id: "missing-id" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.updateAsset({
      ...baseUpdateDto,
      asset_id: "missing-id",
      exchange: null,
    });
    expect(result).toEqual({ status: "error", error: err });
  });

  it("updateAsset surfaces Archived domain leaf", async () => {
    const err: AssetError = { code: "Archived" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.updateAsset(baseUpdateDto);
    expect(result).toEqual({ status: "error", error: err });
  });

  it("updateAsset surfaces CashAssetNotEditable for system Cash Asset", async () => {
    const err: AssetError = { code: "CashAssetNotEditable" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.updateAsset({
      ...baseUpdateDto,
      asset_id: "system-cash-eur",
      exchange: null,
    });
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── unarchiveAsset ─────────────────────────────────────────────────────────

  it("unarchiveAsset returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await assetGateway.unarchiveAsset("asset-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("unarchive_asset", { id: "asset-1" });
  });

  it("unarchiveAsset surfaces CashAssetNotEditable for system Cash Asset", async () => {
    const err: AssetError = { code: "CashAssetNotEditable" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.unarchiveAsset("system-cash-eur");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("unarchiveAsset surfaces NotFound with id payload", async () => {
    const err: AssetError = { code: "AssetNotFound", id: "missing" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.unarchiveAsset("missing");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── archiveAsset ──────────────────────────────────────────────────────────

  it("archiveAsset returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await assetGateway.archiveAsset("asset-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("archive_asset", { id: "asset-1" });
  });

  it("archiveAsset surfaces ActiveHoldings via Application leaf", async () => {
    const err: ArchiveAssetTask = { code: "ActiveHoldings" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.archiveAsset("asset-1");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("archiveAsset surfaces NotFound propagated through Asset leaf", async () => {
    const err: AssetError = { code: "AssetNotFound", id: "missing" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.archiveAsset("missing");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("archiveAsset surfaces DatabaseError from cross-BC Account leaf", async () => {
    const err: AccountError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.archiveAsset("asset-1");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── deleteAsset ───────────────────────────────────────────────────────────

  it("deleteAsset returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await assetGateway.deleteAsset("asset-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("delete_asset", { id: "asset-1" });
  });

  it("deleteAsset surfaces ExistingTransactions via Application leaf", async () => {
    const err: DeleteAssetTask = { code: "ExistingTransactions" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.deleteAsset("asset-1");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("deleteAsset surfaces CashAssetNotEditable propagated through Asset leaf", async () => {
    const err: AssetError = { code: "CashAssetNotEditable" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.deleteAsset("system-cash-eur");
    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("asset gateway — getSupportedExchanges", () => {
  beforeEach(() => vi.clearAllMocks());

  // get_supported_exchanges — infallible, returns Exchange[]
  it("getSupportedExchanges returns the full curated list", async () => {
    const exchanges = [
      { code: "XPAR", label: "Euronext Paris" },
      { code: "XNAS", label: "NASDAQ" },
    ];
    // getSupportedExchanges is infallible: the binding returns the raw array directly
    // (no Result wrapper). The gateway wraps it the same way.
    mockInvoke.mockResolvedValue(exchanges);
    const result = await assetGateway.getSupportedExchanges();
    expect(result).toEqual(exchanges);
    expect(mockInvoke).toHaveBeenCalledWith("get_supported_exchanges");
  });

  it("getSupportedExchanges returns empty list when BE constant is empty", async () => {
    mockInvoke.mockResolvedValue([]);
    const result = await assetGateway.getSupportedExchanges();
    expect(result).toEqual([]);
  });
});

describe("asset gateway — exchange DTO pass-through", () => {
  beforeEach(() => vi.clearAllMocks());

  // createAsset — exchange field forwarded to add_asset
  it("createAsset forwards exchange object in DTO", async () => {
    const asset = makeAsset();
    const exchange = { code: "XPAR", label: "Euronext Paris" };
    mockInvoke.mockResolvedValue(asset);
    const dto: CreateAssetDTO = { ...baseCreateDto, exchange };
    const result = await assetGateway.createAsset(dto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("add_asset", { dto });
  });

  it("createAsset with exchange: null forwards null in DTO", async () => {
    const asset = makeAsset();
    mockInvoke.mockResolvedValue(asset);
    const result = await assetGateway.createAsset(baseCreateDto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("add_asset", { dto: baseCreateDto });
  });

  // createAsset — InvalidExchange error surface (AST-001)
  it("createAsset surfaces InvalidExchange with exchange_code payload", async () => {
    const err: AssetError = { code: "InvalidExchange", exchange_code: "BOGUS" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.createAsset({
      ...baseCreateDto,
      exchange: { code: "BOGUS", label: "" },
    });
    expect(result).toEqual({ status: "error", error: err });
  });

  // updateAsset — exchange field forwarded to update_asset
  it("updateAsset forwards exchange object in DTO", async () => {
    const asset = makeAsset();
    const exchange = { code: "XNAS", label: "NASDAQ" };
    mockInvoke.mockResolvedValue(asset);
    const dto: UpdateAssetDTO = { ...baseUpdateDto, exchange };
    const result = await assetGateway.updateAsset(dto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("update_asset", { dto });
  });

  it("updateAsset with exchange: null clears the exchange field", async () => {
    const asset = makeAsset();
    mockInvoke.mockResolvedValue(asset);
    const result = await assetGateway.updateAsset(baseUpdateDto);
    expect(result).toEqual({ status: "ok", data: asset });
    expect(mockInvoke).toHaveBeenCalledWith("update_asset", { dto: baseUpdateDto });
  });

  // updateAsset — InvalidExchange error surface (AST-001)
  it("updateAsset surfaces InvalidExchange with exchange_code payload", async () => {
    const err: AssetError = { code: "InvalidExchange", exchange_code: "BOGUS" };
    mockInvoke.mockRejectedValue(err);
    const result = await assetGateway.updateAsset({
      ...baseUpdateDto,
      exchange: { code: "BOGUS", label: "" },
    });
    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("asset gateway — lookupAsset", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // WEB-020 — success path returns AssetLookupResult[]
  it("lookupAsset returns result list on success", async () => {
    const results: AssetLookupResult[] = [
      {
        name: "Apple Inc.",
        reference: "AAPL",
        isin: null,
        currency: "USD",
        asset_class: "Stocks",
        exchange: null,
      },
      {
        name: "iShares Core S&P 500",
        reference: "IVV",
        isin: null,
        currency: "USD",
        asset_class: "ETF",
        exchange: null,
      },
    ];
    // bindings.ts wraps the TAURI_INVOKE result in { status: "ok", data: ... }
    mockInvoke.mockResolvedValue(results);

    const res = await assetGateway.lookupAsset("AAPL", "Keyword");

    expect(res).toEqual({ status: "ok", data: results });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", { query: "AAPL", mode: "Keyword" });
  });

  // WEB-020 — ISIN query (12 alphanumeric chars) is forwarded as-is
  it("lookupAsset forwards 12-char ISIN query verbatim", async () => {
    // WEB-046 — on the ISIN path, `reference` is the ticker and `isin` is the
    // normalized ISIN query.
    const results: AssetLookupResult[] = [
      {
        name: "Apple Inc.",
        reference: "AAPL",
        isin: "US0378331005",
        currency: "USD",
        asset_class: "Stocks",
        exchange: null,
      },
    ];
    mockInvoke.mockResolvedValue(results);

    const res = await assetGateway.lookupAsset("US0378331005", "Isin");

    expect(res).toEqual({ status: "ok", data: results });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", {
      query: "US0378331005",
      mode: "Isin",
    });
  });

  // WEB-020 — empty list is a valid success (WEB-032 handled by UI layer)
  it("lookupAsset returns empty list when no instruments found", async () => {
    mockInvoke.mockResolvedValue([]);

    const res = await assetGateway.lookupAsset("xyzzy-not-a-real-ticker", "Keyword");

    expect(res).toEqual({ status: "ok", data: [] });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", {
      query: "xyzzy-not-a-real-ticker",
      mode: "Keyword",
    });
  });

  // WEB-025 — NetworkError is surfaced as { status: "error", error: { code: "NetworkError" } }
  it("lookupAsset returns NetworkError on network failure", async () => {
    const err: WebLookupError = { code: "NetworkError" };
    // bindings.ts catches the rejection and returns { status: "error", error: e }
    mockInvoke.mockRejectedValue(err);

    const res = await assetGateway.lookupAsset("AAPL", "Keyword");

    expect(res).toEqual({ status: "error", error: err });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", { query: "AAPL", mode: "Keyword" });
  });

  // WEB-023/WEB-024/WEB-046 — optional fields may be null
  it("lookupAsset preserves null optional fields from result", async () => {
    const results: AssetLookupResult[] = [
      {
        name: "Obscure Fund",
        reference: null,
        isin: null,
        currency: null,
        asset_class: null,
        exchange: null,
      },
    ];
    mockInvoke.mockResolvedValue(results);

    const res = await assetGateway.lookupAsset("obscure fund", "Keyword");

    expect(res).toEqual({ status: "ok", data: results });
  });

  // WEB-025 — InvalidIsinFormat is surfaced as { status: "error", error: { code: "InvalidIsinFormat" } }
  it("lookupAsset returns InvalidIsinFormat when ISIN path rejects the query", async () => {
    const err: WebLookupError = { code: "InvalidIsinFormat" };
    mockInvoke.mockRejectedValue(err);

    const res = await assetGateway.lookupAsset("NOTANISIN", "Isin");

    expect(res).toEqual({ status: "error", error: { code: "InvalidIsinFormat" } });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", {
      query: "NOTANISIN",
      mode: "Isin",
    });
  });

  // WEB-014 — ISIN mode arg is forwarded exactly as "Isin" (not inferred)
  it("lookupAsset forwards mode: Isin to the backend command", async () => {
    // WEB-046 — on the ISIN path, `reference` is the ticker and `isin` is the
    // normalized ISIN query.
    const results: AssetLookupResult[] = [
      {
        name: "iShares Core S&P 500 UCITS ETF",
        reference: "CSPX",
        isin: "IE00B53L3W79",
        currency: "EUR",
        asset_class: "ETF",
        exchange: null,
      },
    ];
    mockInvoke.mockResolvedValue(results);

    const res = await assetGateway.lookupAsset("IE00B53L3W79", "Isin");

    expect(res).toEqual({ status: "ok", data: results });
    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", {
      query: "IE00B53L3W79",
      mode: "Isin",
    });
  });

  // WEB-014 — Keyword mode arg is forwarded exactly as "Keyword" (not inferred)
  it("lookupAsset forwards mode: Keyword to the backend command", async () => {
    mockInvoke.mockResolvedValue([]);

    await assetGateway.lookupAsset("Apple", "Keyword");

    expect(mockInvoke).toHaveBeenCalledWith("lookup_asset", {
      query: "Apple",
      mode: "Keyword",
    });
  });

  // WEB-025 — RateLimited pass-through still works (regression guard)
  it("lookupAsset returns RateLimited on HTTP 429 from OpenFIGI", async () => {
    const err: WebLookupError = { code: "RateLimited" };
    mockInvoke.mockRejectedValue(err);

    const res = await assetGateway.lookupAsset("AAPL", "Keyword");

    expect(res).toEqual({ status: "error", error: { code: "RateLimited" } });
  });
});
