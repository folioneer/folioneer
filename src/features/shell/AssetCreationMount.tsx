import { useEffect, useState } from "react";
import type { AssetKind } from "@/bindings";
import { AddAssetModal } from "@/features/assets/add_asset/AddAsset";
import { useAssetCreationRequest } from "@/lib/assetCreationRequest";
import { logger } from "@/lib/logger";

/** Shell-level mount of the usual asset form, for a feature that asks for it (IMP-052). */
export function AssetCreationMount() {
  const request = useAssetCreationRequest((state) => state.request);
  const close = useAssetCreationRequest((state) => state.close);
  const [kind, setKind] = useState<AssetKind>("Listed");

  useEffect(() => {
    logger.info("[AssetCreationMount] mounted");
  }, []);

  if (!request) return null;

  return (
    <AddAssetModal
      isOpen
      onClose={close}
      kind={kind}
      onKindChange={setKind}
      prefill={request.prefill}
      onSuccess={request.onCreated}
    />
  );
}
