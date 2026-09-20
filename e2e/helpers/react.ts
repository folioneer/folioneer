import { browser } from "@wdio/globals";

/**
 * Sets a value on a React controlled input by bypassing React's value tracker
 * and dispatching native input/change events. (E2E rule E6)
 *
 * Standard setValue() does NOT reliably trigger React's synthetic onChange in
 * WebKitGTK — the DOM value is set but React state never updates.
 */
export async function setReactInputValue(elementId: string, value: string): Promise<void> {
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
