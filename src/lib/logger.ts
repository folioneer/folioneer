import { invoke } from "@tauri-apps/api/core";

type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

function formatForBackend(...args: unknown[]): string {
  return args
    .map((arg) => {
      if (typeof arg === "object" && arg !== null) {
        try {
          return JSON.stringify(arg);
        } catch {
          return "[Circular or Non-Serializable Object]";
        }
      }
      return String(arg);
    })
    .join(" ");
}

function logToBackend(level: LogLevel, ...args: unknown[]): void {
  const message = formatForBackend(...args);
  invoke("log_frontend", { level, message }).catch(() => {});
}

function createLogger(level: LogLevel) {
  return <T extends unknown[]>(...args: T): void => {
    // console.trace prints a stack trace — use console.log for plain trace output
    const consoleMethod = level === "trace" ? "log" : level;
    console[consoleMethod](...args);
    logToBackend(level, ...args);
  };
}

export const logger = {
  trace: createLogger("trace"),
  debug: createLogger("debug"),
  info: createLogger("info"),
  warn: createLogger("warn"),
  error: createLogger("error"),
};
