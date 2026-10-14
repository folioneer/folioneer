"""Tests for scripts/mcpb-bundle.py (TODO-081) — run with `just test-scripts`."""

import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "mcpb_bundle", Path(__file__).resolve().parents[1] / "mcpb-bundle.py"
)
mcpb_bundle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mcpb_bundle)


class TheBundle(unittest.TestCase):
    """TODO-081 — the bundle names the program as the release installs it."""

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.output = Path(folder.name)
        self.bundle = mcpb_bundle.build(self.output)
        with zipfile.ZipFile(self.bundle) as archive:
            self.names = archive.namelist()
            self.manifest = json.loads(archive.read("manifest.json"))
        conf = json.loads((mcpb_bundle.ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
        self.product, self.version = conf["productName"], conf["version"]

    def test_it_is_named_after_the_release(self):
        self.assertEqual(self.bundle.name, f"{self.product}-{self.version}.mcpb")
        self.assertEqual(self.manifest["version"], self.version)
        self.assertEqual(sorted(self.names), ["icon.png", "manifest.json"])

    def test_the_deb_installs_the_program_on_linux(self):
        config = self.manifest["server"]["mcp_config"]
        self.assertEqual(config["command"], "/usr/bin/folioneer")
        self.assertEqual(config["args"], ["--mcp"])

    def test_the_windows_installer_puts_it_in_the_users_local_programs(self):
        windows = self.manifest["server"]["mcp_config"]["platform_overrides"]["win32"]
        self.assertEqual(windows["command"], "${HOME}\\AppData\\Local\\Folioneer\\folioneer.exe")

    def test_it_is_a_binary_server_for_the_two_systems_released(self):
        self.assertEqual(self.manifest["server"]["type"], "binary")
        self.assertEqual(self.manifest["compatibility"]["platforms"], ["win32", "linux"])
        for field in ("manifest_version", "name", "version", "description", "author", "server"):
            self.assertIn(field, self.manifest)


class TheBinaryName(unittest.TestCase):
    def test_it_is_the_package_name_not_another_tables(self):
        cargo = '[package]\nname = "app"\nversion = "1.0.0"\n\n[lib]\nname = "app_lib"\n'
        self.assertEqual(mcpb_bundle.binary_name(cargo), "app")

    def test_a_package_that_is_the_last_table_is_read(self):
        self.assertEqual(mcpb_bundle.binary_name('[lib]\nname = "l"\n\n[package]\nname = "app"\n'), "app")

    def test_a_cargo_file_without_a_package_is_refused(self):
        with self.assertRaises(ValueError):
            mcpb_bundle.binary_name('[workspace]\nmembers = []\n')


if __name__ == "__main__":
    unittest.main()
