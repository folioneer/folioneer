// CLI-040 — builds `folioneer-cli`, the console program the Windows installer puts beside the
// main one, and places it where Tauri's `externalBin` expects it. Run by Tauri before every
// Windows build (tauri.windows.conf.json), so any build of this source — the public release
// or the owner's private one — ships it. Needs no Tauri: built without default features.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";

const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "x86_64-pc-windows-msvc";
const tauriDir = path.resolve(import.meta.dirname, "..", "src-tauri");
const extension = triple.includes("windows") ? ".exe" : "";

execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "--no-default-features",
    "--manifest-path",
    path.join(tauriDir, "Cargo.toml"),
    "--target",
    triple,
    "--bin",
    "folioneer-cli",
  ],
  { stdio: "inherit" },
);

const built = path.join(tauriDir, "target", triple, "release", `folioneer-cli${extension}`);
const destination = path.join(tauriDir, "binaries", `folioneer-cli-${triple}${extension}`);
mkdirSync(path.dirname(destination), { recursive: true });
copyFileSync(built, destination);
console.log(`folioneer-cli → ${path.relative(process.cwd(), destination)}`);
