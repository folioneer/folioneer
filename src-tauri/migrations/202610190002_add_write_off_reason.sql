-- WOF entity — why a write-off's quantity left the holding; the domain leaves it NULL on every other type.
ALTER TABLE transactions ADD COLUMN write_off_reason TEXT
    CHECK (write_off_reason IS NULL OR write_off_reason IN ('NetworkFee', 'Dust', 'Lost', 'Other'));
