# i18n Rules

Defines translation file structure, what must and must not be translated, and key naming conventions.

## Translation file structure

Translation files live in `src/i18n/locales/{locale}/common.json`, one per language: English (`en`), French (`fr`) and German (`de`). English is the reference every other language follows; `src/i18n/config.ts` lists the languages the window offers.

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

All locale files must carry the same key set. A key present in one locale but missing from another is a Critical finding. `src/i18n/locales.test.ts` holds every language to the English key set.

## Adding a language

A language is a data change, made in this order (owner, 2026-10-17):

1. **An agent translates** `en/common.json` into `{locale}/common.json`, every key as the sections above require; the confirmed terms of `docs/ubiquitous-language.md` keep one translation throughout.
2. The language joins:
   - `SUPPORTED_LANGS` and the resources in `src/i18n/config.ts`;
   - `LANGUAGE_OPTIONS` in `SettingsDialog.tsx` and the `LanguageChoice` type in `useSettings.ts`, with a `settings.language_{locale}` label in every language;
   - the key-set test in `src/i18n/locales.test.ts`;
   - `COMMA_LANGUAGES` in `src/lib/microUnits.ts` when it writes a decimal comma (NUM-001);
   - `I18N_TO_BCP47` in `DateField.tsx`, the locale a date field formats and reads with.
3. **It ships**, agent-translated, in the Settings list.
4. **A reviewer from the owner's circle** reads it in the window; the corrections ride a later release.
5. **The community** takes it over once the translation is opened to contributions (not yet set up).
