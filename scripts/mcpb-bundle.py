#!/usr/bin/env python3
"""mcpb-bundle.py — the Claude Desktop bundle each release carries (TODO-081, ADR-023).

Claude Desktop installs a local server from a bundle (`.mcpb`): a zip holding a
`manifest.json`, opened by a double-click and installed by one confirmation. Folioneer's
bundle holds the manifest and an icon only: its server is the program the release already
installs, started with `--mcp` — the bridge of ADR-023, unchanged.

Where that program is, as the release installs it:
  - the Windows installer (NSIS, for the current user): `%LOCALAPPDATA%\\<productName>\\
    <binary>.exe`, written from `${HOME}` since the manifest knows no `%LOCALAPPDATA%`;
  - the `.deb`: `/usr/bin/<binary>`.
The AppImage is wherever its owner saved it: it keeps the block of JSON (README § Agents).

The product name and the version are read from `src-tauri/tauri.conf.json`, the binary's
name from `src-tauri/Cargo.toml`, so the bundle follows the release it is built for.

Use: python3 scripts/mcpb-bundle.py <output directory>   (writes <productName>-<version>.mcpb)
"""

from __future__ import annotations

import json
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ICON = Path("src-tauri") / "icons" / "128x128.png"


def binary_name(cargo_toml: str) -> str:
    """The `[package]` name of the application's crate: the installed program's name."""
    package = re.search(r"^\[package\]\s*$(.*?)(?=^\[|\Z)", cargo_toml, re.M | re.S)
    if not package:
        raise ValueError("Cargo.toml has no [package] table")
    name = re.search(r'^name\s*=\s*"([^"]+)"', package.group(1), re.M)
    if not name:
        raise ValueError("Cargo.toml [package] has no name")
    return name.group(1)


def manifest(product: str, version: str, binary: str) -> dict:
    """The bundle's manifest: the installed program, started with `--mcp`."""
    linux = f"/usr/bin/{binary}"
    windows = f"${{HOME}}\\AppData\\Local\\{product}\\{binary}.exe"
    return {
        "manifest_version": "0.3",
        "name": binary,
        "display_name": product,
        "version": version,
        "description": f"Lets Claude read and record in your {product} portfolio while "
        f"{product} is open, after you allow it there.",
        "author": {"name": product},
        "icon": "icon.png",
        "server": {
            "type": "binary",
            "entry_point": linux,
            "mcp_config": {
                "command": linux,
                "args": ["--mcp"],
                "env": {},
                "platform_overrides": {"win32": {"command": windows}},
            },
        },
        "compatibility": {"platforms": ["win32", "linux"]},
    }


def build(output: Path, root: Path = ROOT) -> Path:
    """Writes `<productName>-<version>.mcpb` into `output` and returns its path."""
    conf = json.loads((root / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    product, version = conf["productName"], conf["version"]
    binary = binary_name((root / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8"))
    output.mkdir(parents=True, exist_ok=True)
    bundle = output / f"{product}-{version}.mcpb"
    with zipfile.ZipFile(bundle, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest(product, version, binary), indent=2))
        archive.write(root / ICON, "icon.png")
    return bundle


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("use: python3 scripts/mcpb-bundle.py <output directory>")
    print(build(Path(sys.argv[1])))
