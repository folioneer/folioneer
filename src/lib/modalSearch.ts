import type { useNavigate } from "@tanstack/react-router";

type NavigateFn = ReturnType<typeof useNavigate>;

/**
 * Shell-level URL params that drive modal mounting (consumed by
 * AssetEditModalMount). They are absent from every route's `validateSearch`
 * so they don't cascade typing onto the rootRoute and every child route.
 */
export type ModalSearchParams = {
  modal?: string;
  editAssetId?: string;
  focusField?: string;
  /** Cash-transaction edit (CSH-111): the transaction id + the (account, asset) needed to refetch it. */
  editTxId?: string;
  editTxAccountId?: string;
  editTxAssetId?: string;
  /** Record-FX-rate shortcut (FXR-012): the directed pair to pre-fill (`modal=record-fx-rate`). */
  fxFrom?: string;
  fxTo?: string;
  /** Add-transaction modal (ACD-035/036): account to pre-fill (`modal=add-transaction`). */
  prefillAccountId?: string;
};

/**
 * Merge `patch` into the current URL search params via `navigate`. This is the
 * single boundary that casts past per-route `validateSearch` typing for the
 * untyped shell-level modal params; call sites stay fully typed.
 */
export function patchModalSearch(
  navigate: NavigateFn,
  patch: ModalSearchParams,
  options?: { replace?: boolean },
): void {
  navigate({
    // biome-ignore lint/suspicious/noExplicitAny: shell-level modal params bypass per-route validateSearch typing
    search: ((prev: Record<string, unknown>) => ({ ...prev, ...patch })) as any,
    replace: options?.replace,
  });
}
