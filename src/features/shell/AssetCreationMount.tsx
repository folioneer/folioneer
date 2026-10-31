import { useEffect, useState } from "react";
import type { AssetKind } from "@/bindings";
import { AddAssetModal } from "@/features/assets/add_asset/AddAsset";
import { type AssetCreationRequest, useAssetCreationRequest } from "@/lib/assetCreationRequest";
import { logger } from "@/lib/logger";

/** Shell-level mount of the usual asset form, for a feature that asks for it (IMP-052). */
export function AssetCreationMount() {
  const request = useAssetCreationRequest((state) => state.request);
  const close = useAssetCreationRequest((state) => state.close);

  useEffect(() => {
    logger.info("[AssetCreationMount] mounted");
  }, []);

  if (!request) return null;

  return <AssetCreationForm request={request} onClose={close} />;
}

/** One request's form: a Listed asset needs an ISIN, so a request without one starts Custom. */
function AssetCreationForm({
  request,
  onClose,
}: {
  request: AssetCreationRequest;
  onClose: () => void;
}) {
  const [kind, setKind] = useState<AssetKind>(request.prefill.isin ? "Listed" : "Custom");
  return (
    <AddAssetModal
      isOpen
      onClose={onClose}
      kind={kind}
      onKindChange={setKind}
      prefill={request.prefill}
      onSuccess={request.onCreated}
    />
  );
}
