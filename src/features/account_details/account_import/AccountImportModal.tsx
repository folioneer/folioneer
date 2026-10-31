import { AccountImportDialog } from "./AccountImportDialog";
import { useAccountImport } from "./useAccountImport";

interface AccountImportModalProps {
  onClose: () => void;
  accountId: string;
  accountName: string;
}

/** The import window, mounted while open: each opening starts from picking a file (IMP-060). */
export function AccountImportModal({ onClose, accountId, accountName }: AccountImportModalProps) {
  const view = useAccountImport(accountId);
  return <AccountImportDialog view={view} onClose={onClose} accountName={accountName} />;
}
