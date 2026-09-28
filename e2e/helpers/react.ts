import { browser } from "@wdio/globals";

/**
 * Sets a value on a React controlled input by bypassing React's value tracker
 * and dispatching native input/change events. (E2E rule E6)
 *
 * Standard setValue() does NOT reliably trigger React's synthetic onChange in
 * WebKitGTK — the DOM value is set but React state never updates.
 *
 * L-019 — the synthetic event is not always taken either: React then re-renders
 * the field with its old value and the typed one vanishes. So after setting, the
 * helper waits for the page to settle and reads the field back; a field that no
 * longer shows the value is logged with what the page looked like and typed once
 * more, so a later wait never fails far from its cause.
 */
export async function setReactInputValue(elementId: string, value: string): Promise<void> {
  await dispatchValue(elementId, value);
  await browser.pause(SETTLE_MS);
  const shown = await readField(elementId);
  if (shown.value === value) return;
  console.warn(
    `[setReactInputValue] #${elementId} shows "${shown.value}" instead of "${value}" ` +
      `(connected: ${shown.connected}, focus: ${shown.focus}) — typing it again`,
  );
  await dispatchValue(elementId, value);
  await browser.pause(SETTLE_MS);
  const again = await readField(elementId);
  if (again.value !== value) {
    console.warn(
      `[setReactInputValue] #${elementId} still shows "${again.value}" instead of "${value}"`,
    );
  }
}

/** Long enough for a draft check or snapshot answer to re-render the form. */
const SETTLE_MS = 300;

async function dispatchValue(elementId: string, value: string): Promise<void> {
  await browser.execute(
    (id, val) => {
      const el = document.getElementById(id) as HTMLInputElement | null;
      if (!el) return;
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value",
      )?.set;
      nativeSetter?.call(el, val);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    },
    elementId,
    value,
  );
}

async function readField(
  elementId: string,
): Promise<{ value: string; connected: boolean; focus: string }> {
  return browser.execute((id) => {
    const el = document.getElementById(id) as HTMLInputElement | null;
    const active = document.activeElement;
    return {
      value: el ? el.value : "<missing>",
      connected: el ? el.isConnected : false,
      focus: active ? active.id || active.tagName : "none",
    };
  }, elementId);
}

/**
 * Textarea variant of `setReactInputValue` — React's value tracker hangs off
 * `HTMLTextAreaElement.prototype`, so the input-element setter cannot drive a
 * controlled `<textarea>`. (E2E rule E6)
 */
export async function setReactTextareaValue(elementId: string, value: string): Promise<void> {
  await browser.execute(
    (id, val) => {
      const el = document.getElementById(id) as HTMLTextAreaElement | null;
      if (!el) return;
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      nativeSetter?.call(el, val);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    },
    elementId,
    value,
  );
}
