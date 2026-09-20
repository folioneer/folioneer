import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetCategory, AssetError } from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const { categoryGateway } = await import("./gateway");

const makeCategory = (): AssetCategory => ({
  id: "cat-1",
  name: "Equities",
});

describe("categoryGateway", () => {
  beforeEach(() => vi.clearAllMocks());

  // ── getCategories ─────────────────────────────────────────────────────────────

  it("getCategories returns list on success", async () => {
    const categories = [makeCategory()];
    mockInvoke.mockResolvedValue(categories);
    const result = await categoryGateway.getCategories();
    expect(result).toEqual({ status: "ok", data: categories });
    expect(mockInvoke).toHaveBeenCalledWith("get_categories");
  });

  it("getCategories surfaces DatabaseError on repo failure", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.getCategories();
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── addCategory ───────────────────────────────────────────────────────────────

  it("addCategory returns category on success", async () => {
    const category = makeCategory();
    mockInvoke.mockResolvedValue(category);
    const result = await categoryGateway.addCategory("Equities");
    expect(result).toEqual({ status: "ok", data: category });
    expect(mockInvoke).toHaveBeenCalledWith("add_category", {
      label: "Equities",
    });
  });

  it("addCategory surfaces DuplicateName from application leaf", async () => {
    const err: AssetError = { code: "DuplicateName" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.addCategory("Equities");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("addCategory surfaces LabelEmpty from domain leaf", async () => {
    const err: AssetError = { code: "LabelEmpty" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.addCategory("   ");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("addCategory surfaces DatabaseError when repo write fails", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.addCategory("Equities");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── updateCategory ────────────────────────────────────────────────────────────

  it("updateCategory returns updated category on success", async () => {
    const updated = { ...makeCategory(), name: "Fixed Income" };
    mockInvoke.mockResolvedValue(updated);
    const result = await categoryGateway.updateCategory("cat-1", "Fixed Income");
    expect(result).toEqual({ status: "ok", data: updated });
    expect(mockInvoke).toHaveBeenCalledWith("update_category", {
      id: "cat-1",
      label: "Fixed Income",
    });
  });

  it("updateCategory surfaces SystemReadonly from domain leaf", async () => {
    const err: AssetError = { code: "SystemReadonly" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.updateCategory("cat-default", "New Name");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("updateCategory surfaces CategoryNotFound with id payload", async () => {
    const err: AssetError = { code: "CategoryNotFound", id: "missing-id" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.updateCategory("missing-id", "Anything");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── deleteCategory ────────────────────────────────────────────────────────────

  it("deleteCategory returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await categoryGateway.deleteCategory("cat-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("delete_category", { id: "cat-1" });
  });

  it("deleteCategory surfaces SystemProtected from domain leaf", async () => {
    const err: AssetError = { code: "SystemProtected" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.deleteCategory("cat-default");
    expect(result).toEqual({ status: "error", error: err });
  });

  it("deleteCategory surfaces DatabaseError when repo cascade fails", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await categoryGateway.deleteCategory("cat-1");
    expect(result).toEqual({ status: "error", error: err });
  });
});
