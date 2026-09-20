const KEY = "vaultcompass.lastPath";
const DEFAULT_PATH = "/accounts";
const RESTORABLE_PATHS = ["/accounts", "/assets", "/categories"] as const;

type RestorablePath = (typeof RESTORABLE_PATHS)[number];

function toTopLevel(pathname: string): RestorablePath | null {
  return RESTORABLE_PATHS.find((p) => pathname === p || pathname.startsWith(`${p}/`)) ?? null;
}

export function saveLastPath(pathname: string): void {
  const top = toTopLevel(pathname);
  if (top) localStorage.setItem(KEY, top);
}

export function getLastPath(): RestorablePath {
  const saved = localStorage.getItem(KEY);
  return (RESTORABLE_PATHS as readonly string[]).includes(saved ?? "")
    ? (saved as RestorablePath)
    : DEFAULT_PATH;
}
