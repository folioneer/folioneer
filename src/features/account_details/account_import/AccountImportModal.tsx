import { AccountImportDialog } from "./AccountImportDialog";
import { useAccountImport } from "./useAccountImport";

interface AccountImportModalProps {
  onClose: () => void;
  accountId: string;
  accountName: string;
  accountCurrency: string;
}

/** The import window, mounted while open: each opening starts from picking a file (IMP-060). */
export function AccountImportModal({
  onClose,
  accountId,
  accountName,
  accountCurrency,
}: AccountImportModalProps) {
  const view = useAccountImport(accountId, accountCurrency, onClose);
  return (
    <AccountImportDialog
      view={view}
      onClose={onClose}
      accountName={accountName}
      accountCurrency={accountCurrency}
    />
  );
}
