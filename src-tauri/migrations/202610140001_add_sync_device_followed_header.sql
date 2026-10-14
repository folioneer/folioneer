-- What else a device keeps of the folder header it follows (SYN-084, SYN-074): the device
-- that wrote the header, so two headers written at the same logical clock are told apart,
-- and the key's derivation parameters, so a move needs nothing of the folder it leaves.
-- Device-local, never published. NULL on a device enrolled before: it is then known by
-- its creation mark alone, and a move reads the parameters from its folder.
ALTER TABLE sync_device ADD COLUMN portfolio_created_by TEXT;
ALTER TABLE sync_device ADD COLUMN derivation_parameters TEXT;
