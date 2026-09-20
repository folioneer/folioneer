# i18n Rules

Defines translation file structure, what must and must not be translated, and key naming conventions.

## Translation file structure

Translation files live in `src/infra/i18n/locales/{locale}/` (the i18n runtime adapter lives under the `infra/` bucket per F0). Each locale directory contains JSON files organized by domain (e.g. `common.json`, `auth.json`). Locale names are discovered at review time — do not hardcode them.

## What must be translated

All user-visible text in `.tsx` files must use `t("key")` from the i18n library. This includes:

- Button labels and action text
- Placeholder text in inputs
- Error messages and validation feedback
- Column headers and table labels
- Page titles and section headings
- Tooltip content

## What does NOT need translation

- Variable names, comments
- Logger and console calls
- `className` strings
- `id` and `data-*` attributes
- Date/time format strings (`"yyyy-MM-dd"`, etc.)
- URLs and file paths

## Key naming convention

Dot notation with **snake_case for every segment**: `{domain}.{component}.{element}` — e.g. `auth.login_form.submit_button`, `invoice.table.amount_header`. snake_case keeps keys readable in JSON and consistent with backend identifiers; never mix camelCase or kebab-case segments.

## Cross-locale requirement

All locale files must carry the same key set. A key present in one locale but missing from another is a Critical finding.
