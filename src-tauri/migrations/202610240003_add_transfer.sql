-- TRF entity — the transfer a transaction belongs to: its two sides and its fee in kind
-- share it; the domain leaves it NULL on every other transaction. The transfer kinds
-- (`TransferOut`, `TransferIn`) are values of the unchecked `transaction_type` column.
ALTER TABLE transactions ADD COLUMN transfer_id TEXT;
CREATE INDEX IF NOT EXISTS idx_transactions_transfer_id ON transactions (transfer_id)
    WHERE transfer_id IS NOT NULL;
