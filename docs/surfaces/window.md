# The window — its screens and their mocks

The window's description is its validated mocks, one folder per issue in
[`folioneer/folioneer-design`](https://github.com/folioneer/folioneer-design)
(`docs/workflow.md` § Design proposal). This index finds them: one heading per screen,
named as its E2E screenshot (`e2e/helpers/screenshot.ts`; each pull request's
comment shows them), with the mocks that set it — latest first, issue and folder —
and the spec rules it shows. A screen no screenshot covers yet is named by what it
shows, and says so.

A window rule that leaves a spec cites its screen here (`docs/spec/README.md`); a new
validated mock joins its screen as `docs/workflow.md` § Design proposal says.

## accounts-list

- Mocks: #769 `769/` (holder column), #767 `767/` (one profile, the switcher), #405
  `405/` (no update frequency), #386 `386/` (an account an AI agent created).
- Rules: ACC-007 to ACC-014, ACC-023, ACC-024, ACC-030 to ACC-033, ACC-046; AGT-069.

## account-form

No E2E screenshot yet: the create and edit dialogs of an account.

- Mocks: #769 `769/` (holder), #767 `767/` (profile, the move warning), #515 `515/`
  (cost basis method), #405 `405/` (no update frequency).
- Rules: ACC-016, ACC-017, ACC-026, ACC-045, ACC-048; the core's CBM-011A, CBM-030 and
  PFL-031 (their window part is the mocks).

## account-details-holdings

- Mocks: #864 `864/` (the header's Global Value, as of a date), #734 `734/` (the
  header), #712 `712/` (row actions and hints), #551 `551/` (a removal on the row),
  #529 `529/` (a split the provider reports), #515 `515/` (a holding's lots).
- Rules: ACD-010 to ACD-012, ACD-034 to ACD-042, ACD-047 to ACD-052A; HNO-041,
  HNO-042; FEE-074 to FEE-076.
- Variants: `account-details-holdings-scrolled`, `account-details-refresh`.

## account-details-cash

- Mocks: #864 `864/` (the header).
- Rules: CSH-019, CSH-091 to CSH-098, CSH-110.

## account-details-dividend

- Mocks: #734 `734/` (the form: same currency, foreign, a refusal).
- Rules: DIV-026, DIV-041 to DIV-044, DIV-050, DIV-072.

## account-details-split

- Mocks: #529 `529/` (the provider's split notice).
- Rules: SPL-040, SPL-060, SPL-061, SPL-064.

## account-details-management-fee

- Mocks: none yet.
- Rules: FEE-011, FEE-020 to FEE-034, FEE-052 to FEE-064.
- Variant: `account-details-management-fee-by-quantity`.

## account-performance-past-year

- Mocks: none yet.
- Rules: PRF-010 to PRF-016, PRF-036 to PRF-053, PRF-060, PRF-072 to PRF-075, PRF-088.

## account-import-pick-file

- Mocks: #503 `503/` (pick a file, a new mapper, a mismatch, a mapper file refused).
- Rules: IMP-020D, IMP-060 to IMP-064.

## account-import-preview

- Mocks: #504 `504/` (the preview, the same file again).
- Rules: IMP-051B, IMP-065, IMP-066, IMP-068.

## account-import-create-asset

- Mocks: #504 `504/` (an unknown asset).
- Rules: IMP-052, IMP-052A, IMP-067.

## account-import-journal

- Mocks: #504 `504/` (the journal).
- Rules: IMP-054.

## free-shares-transactions

- Mocks: none yet.
- Rules: FSD-020, FSD-021, FSD-025, FSD-026, FSD-040, FSD-041, FSD-050, FSD-051.

## open-balance-form

- Mocks: none yet.
- Rules: TRX-043 to TRX-046, TRX-051, TRX-053, TRX-057, TRX-058, TRX-065.

## transfer-form

No E2E screenshot yet: a transfer from a holding row, and in the journals.

- Mocks: #511 `511/` (the form, between currencies, the journals).
- Rules: TRF-050 to TRF-057.

## write-off-form

No E2E screenshot yet: a removal without proceeds.

- Mocks: #551 `551/` (the form, lost shares, a refusal, the row).
- Rules: WOF-050 to WOF-059.

## holding-note-alarm

- Mocks: none yet.
- Rules: HNO-041, HNO-042.

## assets-list

- Mocks: none yet.
- Rules: CSH-015, CSH-016.

## asset-edit-modal

- Mocks: none yet.
- Rules: AST-021, AST-038 to AST-041.

## asset-web-lookup

- Mocks: none yet.
- Rules: WEB-010 to WEB-047.

## currency-pairs

- Mocks: none yet.
- Rules: FXR-012 to FXR-036, FXR-051 to FXR-055, FXR-075, FXR-090, FXR-091, FXR-110.

## header-price-freshness

- Mocks: none yet.
- Rules: MKT-115, MKT-130, MKT-133, MKT-181, MKT-202.

## price-movement-dialog

- Mocks: none yet.
- Rules: PMV-010 to PMV-053.

## unpriced-prices-modal

- Mocks: none yet.
- Rules: MKT-170 to MKT-172.

## price-history-backfill-unresolved-ticker

- Mocks: none yet.
- Rules: MKT-196.

## price-provider-on

- Mocks: #568 `568/` (the Prices section, its notice).
- Rules: SPF-010, SPF-052, SPF-060, SPF-061.
- Variants: `price-provider-off`, `price-provider-notice`, `scheduled-fetch-settings`.

## settings-general

No E2E screenshot yet: the General section of the Settings dialog.

- Mocks: #568 `568/` (the dialog's sections).
- Rules: FXR-129 to FXR-131.

## sync-section

- Mocks: #569 `569/` (sync off, on, scrolled).
- Rules: SYN-010 to SYN-019, SYN-053, SYN-054, SYN-061, SYN-063, SYN-066, SYN-069 to
  SYN-075.
- Variant: `sync-rename-over-settings` (#569 `569/`, the rename).

## profiles

No E2E screenshot yet: the profile switcher in the header and the profiles overview.

- Mocks: #768 `768/` (the overview), #767 `767/` (the switcher, managing profiles).
- Rules: the core's PFL-032, PFL-040, PFL-055 (their window part is the mocks).

## agent-session

No E2E screenshot yet: an AI agent's connection, its session and its request dialogs.

- Mocks: #770 `770/` (the profile chosen at approval), #513 `513/` (a replacement
  request), #386 `386/` (connection, session).
- Rules: AGT-022, AGT-023, AGT-033, AGT-034, AGT-036, AGT-039, AGT-044, AGT-045,
  AGT-054, AGT-083, AGT-084, AGT-089, AGT-092.

## export

No E2E screenshot yet: exporting accounts, assets, transactions and prices.

- Mocks: #509 `509/` (exported, failed).

## whats-new

No E2E screenshot yet.

- Mocks: #674 `674/` (each change's application area).
- Rules: WNW-010 to WNW-092.

## update-banner

No E2E screenshot yet: the update banner and the manual check.

- Mocks: #873 `873/` (available again, check failed, install failed, ready kept).
- Rules: UPD-031, UPD-035.
