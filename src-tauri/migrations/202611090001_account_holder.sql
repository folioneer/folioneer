-- ACC-050 — whose name an account is in: free text, none when unset. Read by no figure.
ALTER TABLE accounts ADD COLUMN holder TEXT;
