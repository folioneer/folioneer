import { useEffect, useMemo, useRef, useState } from "react";

/** The answer of a check: the shape every generated command returns. */
export type CheckResult<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

/** What the check answered for the latest request. */
export interface LatestCheck<T, E> {
  /** The answer's data; null until the latest request is answered ok. */
  data: T | null;
  /** The answer's error; null unless the latest request is answered with one. */
  error: E | null;
  /** The check itself threw for the latest request. */
  failed: boolean;
}

/**
 * Runs `check` each time `request` changes (compared by value) and keeps only the answer
 * to the latest request; an earlier answer arriving late is dropped. `null` runs nothing.
 * While a request is unanswered, data and error are null and failed is false.
 */
export function useLatestCheck<R, T, E>(
  request: R | null,
  check: (request: R) => Promise<CheckResult<T, E>>,
  onFailure?: (cause: unknown) => void,
): LatestCheck<T, E> {
  const key = JSON.stringify(request);
  const latest = useRef(0);
  const checkRef = useRef(check);
  checkRef.current = check;
  const onFailureRef = useRef(onFailure);
  onFailureRef.current = onFailure;
  const [answer, setAnswer] = useState<{ key: string; result: LatestCheck<T, E> } | null>(null);

  // `key` stands for `request` by value: the effect reruns only when the request changes.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `request` is compared by value through `key`
  useEffect(() => {
    const sequence = ++latest.current;
    if (request === null) return;
    checkRef.current(request).then(
      (res) => {
        if (sequence !== latest.current) return;
        setAnswer({
          key,
          result:
            res.status === "ok"
              ? { data: res.data, error: null, failed: false }
              : { data: null, error: res.error, failed: false },
        });
      },
      (cause: unknown) => {
        if (sequence !== latest.current) return;
        onFailureRef.current?.(cause);
        setAnswer({ key, result: { data: null, error: null, failed: true } });
      },
    );
  }, [key]);

  const current = answer?.key === key ? answer.result : null;
  return useMemo(() => current ?? { data: null, error: null, failed: false }, [current]);
}
