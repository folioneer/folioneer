import { useCallback, useMemo, useState } from "react";

/** Where a form shows why it cannot be saved. At most one of the three is set. */
export interface ProblemPlacement<Field extends string, Message> {
  /** The problem as an error on the field it concerns, once the user has typed in it. */
  fieldErrors: Partial<Record<Field, Message>>;
  /** The field the problem concerns, while the user has not typed in it: a plain hint. */
  untouchedField: Field | null;
  /** The problem as an error beside the actions: it concerns no field. */
  alert: Message | null;
  /** Records that the user typed in a field. */
  touch: (field: Field) => void;
}

/**
 * Places a form's first problem: an error on its field once the user has typed in it, a
 * hint naming the field until then, an error beside the actions when it concerns no field.
 * `message` null means no problem; `field` null means the problem concerns no field.
 */
export function useProblemPlacement<Field extends string, Message>(
  field: Field | null,
  message: Message | null,
): ProblemPlacement<Field, Message> {
  const [touched, setTouched] = useState<ReadonlySet<Field>>(() => new Set());
  const touch = useCallback((typed: Field) => {
    setTouched((previous) => (previous.has(typed) ? previous : new Set(previous).add(typed)));
  }, []);

  return useMemo(() => {
    const nothing: ProblemPlacement<Field, Message> = {
      fieldErrors: {},
      untouchedField: null,
      alert: null,
      touch,
    };
    if (message === null) return nothing;
    if (field === null) return { ...nothing, alert: message };
    if (!touched.has(field)) return { ...nothing, untouchedField: field };
    return { ...nothing, fieldErrors: { [field]: message } as Partial<Record<Field, Message>> };
  }, [field, message, touched, touch]);
}
