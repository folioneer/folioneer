interface FormProblemProps {
  /** Prefix of the element ids: `{idPrefix}-error` and `{idPrefix}-hint`. */
  idPrefix: string;
  /** Why saving failed or cannot happen, when it concerns no field. Shown as an error. */
  error?: string;
  /** What to enter to make saving possible. Shown plainly, and only without an error. */
  hint?: string;
}

/**
 * Why a form cannot be saved, beside its actions and always in view: an error, or a plain
 * hint for a field the user has not typed in yet. Renders nothing when there is neither.
 * Sits first in a footer row and pushes the actions to the right.
 */
export function FormProblem({ idPrefix, error, hint }: FormProblemProps) {
  if (error) {
    return (
      <p id={`${idPrefix}-error`} role="alert" className="mr-auto text-sm text-m3-error">
        {error}
      </p>
    );
  }
  if (hint) {
    return (
      <p
        id={`${idPrefix}-hint`}
        role="status"
        className="mr-auto text-sm text-m3-on-surface-variant"
      >
        {hint}
      </p>
    );
  }
  return null;
}
