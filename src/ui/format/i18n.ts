/**
 * F27 layer-3 presenter output type: an i18n key plus optional interpolation vars.
 *
 * Returned by every per-BC `*MutationErrorToI18n` presenter; consumed by every
 * intermediate hook and component that needs to render a localised error string
 * via `t(message.key, message.vars)`. Lives under `ui/format/` per F28 because
 * it's a cross-feature primitive — not feature-owned, not bounded-context-scoped.
 */
export type I18nMessage = {
  key: string;
  vars?: Record<string, string | number>;
};

/**
 * The text of a message in the current language, or `undefined` when there is none —
 * what a field's `error` prop and a hint take. `translate` is react-i18next's `t`.
 */
export function messageText(
  translate: (key: string, vars?: Record<string, string | number>) => string,
  message: I18nMessage | null | undefined,
): string | undefined {
  return message ? translate(message.key, message.vars) : undefined;
}

/**
 * Variant of `I18nMessage` returned by presenters whose output drives a global
 * snackbar instead of component-rendered error state. Carries the i18n key + vars
 * plus a severity dimension the snackbar needs.
 *
 * `severity` is intentionally narrower than `SnackbarVariant` in @/ui/components/snackbar/snackbarStore
 * (no `"success"`) because error presenters never return success — narrowing here
 * documents that constraint at the type level.
 */
export type SnackbarMessage = I18nMessage & { severity: "info" | "error" };
