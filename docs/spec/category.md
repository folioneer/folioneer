# Business Rules — Asset Category Management

## Context

A category (`AssetCategory`) is a free-form grouping defined by the user to organize assets (e.g. "US Stocks", "European Real Estate", "Short-term Bonds"). It is not a fixed taxonomy: the user creates, renames, and deletes their own categories. Categories are used in the performance dashboard to aggregate values per group. A system category (`default-uncategorized`) exists permanently as a fallback when no category is chosen.

---

## Category field definitions

| Field  | Meaning                                                                                                                          |
| ------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `id`   | Unique identifier generated at creation (UUID). The system category has the fixed id `default-uncategorized`.                    |
| `name` | User-defined human-readable name (e.g. "US Stocks", "European Real Estate"). Unique across active categories (case-insensitive). |

---

## Business rules

### Backend

**CAT-001 (was R1) — Required fields (backend)**: A category is valid if its `name` is non-empty and unique among all existing categories (case-insensitive). The backend rejects creation or modification with an already-used name.

**CAT-002 (was R2) — System category cannot be deleted or renamed (backend)**: The category `default-uncategorized` (id: `default-uncategorized`) is a pre-seeded system category. It cannot be deleted or renamed. Any attempt is rejected by the backend with an explicit error.

**CAT-003 (was R3) — Deletion with atomic reassignment (backend)**: Deleting a non-system category reassigns all linked assets to `default-uncategorized` within the same SQL transaction. If the reassignment fails, the deletion is rolled back. Deletion is always allowed (no upfront block).

### Frontend

**CAT-004 (was R4) — Category table — columns (frontend)**: The table displays the following columns, sorted by Name ascending by default:

| Column  | Content                                                                       | Sortable |
| ------- | ----------------------------------------------------------------------------- | -------- |
| Name    | `category.name` + "Default" badge if system category                          | Yes      |
| Actions | Edit button (always visible) + Delete button (hidden for the system category) | No       |

A header displays the title "Categories", the total category count, and a search field filtering by name.

**CAT-005 (was R5) — System category visibility (frontend)**: `default-uncategorized` is visible in the list with a translated badge ("Defaut" / "Default"). The Delete button is visible but disabled. The Edit button is visible but disabled.

**CAT-006 (was R6) — Creation via FAB (frontend)**: A floating FAB at the bottom right opens a creation modal with a single Name field (required). Submission is blocked if the name is empty. If the backend returns a duplicate error, an error message is displayed inline in the modal.

**CAT-007 (was R7) — Modification (frontend)**: The Edit button opens a modal with the Name field pre-filled. After save, the modal closes and the table refreshes. If the backend returns an error (duplicate or system category), an error message is displayed inline.

**CAT-008 (was R8) — Deletion (frontend)**: The Delete button opens a confirmation dialog stating that linked assets will be moved to "Uncategorized". Confirmation triggers the deletion and atomic reassignment (CAT-003).

**CAT-009 (was R9) — Error states (frontend)**: Any backend call failure (creation, modification, deletion) displays an inline error message in the active modal or dialog. The table exposes an error state with a Retry button if the initial load fails.

---

## Workflow

```
[User opens "Categories"]
  → Table (default sort: Name asc) + FAB
          │
          ├─ [Search] → Real-time filter by name
          ├─ [Click on Name header] → Ascending/descending sort
          │
          ├─ [FAB] → Creation modal (Name field)
          │            → Inline error if name empty or duplicate
          │            → Category created → modal closed → table refreshed
          │
          ├─ [Edit] → Edit modal (Name pre-filled)
          │   [disabled if system]
          │            → Inline error if duplicate or system
          │            → Modification → modal closed → table refreshed
          │
          └─ [Delete] → Dialog (mentions reassignment to default)
          [disabled if system]
                            → Confirmation → Deletion + atomic reassignment
```

---

## UX Mockup

### Entry point

**Categories** — item in the main navigation drawer.

### Main component

Full-width table page, sorted by Name ascending by default. Floating FAB at the bottom right. Edit and Delete buttons on each row, both disabled for the system category.

### States

- **Empty**: impossible (the system category is always present)
- **Loading**: Loading indicator in the table
- **Load error**: Error message + Retry button
- **Duplicate error**: Inline message in the modal "This name is already in use."
- **Delete confirmation**: Dialog "Linked assets will be moved to 'Uncategorized'."
- **Backend error**: Inline error message in the active modal or dialog

### User flow

1. The user opens the Categories page.
2. The system category appears with a "Default" badge, both Edit and Delete buttons disabled.
3. They click the FAB → modal → enter a name → submit → category created.
4. They click Edit (custom category) → pre-filled modal → modify → save.
5. They click Delete → dialog (mentions reassignment) → confirm → deletion + atomic reassignment.

---

## Test waivers

**CAT-005, CAT-008, CAT-009 — `CategoryTable` conditional rendering not tested**: These rules involve only React conditional rendering (badge/disabled/hidden, dialog opening, error display). The underlying business logic is covered by existing tests:

- CAT-005: `isSystemCategory()` is a pure function tested indirectly via backend service tests (CAT-002);
- CAT-008: the atomic delete operation is covered by `delete_category_reassigns_assets_to_default` (service.rs);
- CAT-009: Add/Edit error paths are covered by `useAddCategory.test.ts` and `useEditCategoryModal.test.ts`.

A full RTL component test would require mocking the Zustand store and the gateway — disproportionate cost to verify JSX ternaries with no logic.

## Open questions

None — all questions have been resolved.

## Cross-amendments

- **CFR-030** — an asset whose category was removed on another device is shown in the default category, derived on read; CAT-003's reassignment is never re-run on apply.
- **CFR-035** — after a merge two categories may carry the same name; CAT-001's uniqueness check binds the name being set by the user, not names that already clash (see `sync-conflict-resolution.md`).
