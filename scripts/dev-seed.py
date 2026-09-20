#!/usr/bin/env python3
"""Seed the development data folder with a copy of the installed application's database.

Usage: just dev-seed [--replace]

The installed application's folder is only ever read: its files are copied, and every
change is made to the copy. The copy is detached from what would make a development run
act as the real computer — its membership of the sync folder is removed and its daily
price fetch is disabled. Close the installed application first: files copied while it
writes can form a torn copy, which the integrity check then refuses.
"""

import argparse
import os
import shutil
import sqlite3
import sys
from pathlib import Path

INSTALLED_IDENTIFIER = "com.folioneer.desktop"
DEVELOPMENT_IDENTIFIER = "com.folioneer.desktop.dev"
DATABASE_FILENAME = "portfolio"
# The write-ahead log holds the latest writes until a checkpoint; the copy needs it too.
COPIED_SUFFIXES = ("", "-wal")
DATABASE_SUFFIXES = ("", "-wal", "-shm")


class SeedRefused(Exception):
    """The seed cannot run as asked; the message says why."""


def platform_data_folder():
    """The folder the application's data folders sit under, as the `dirs` crate resolves it."""
    if sys.platform == "win32":
        local_app_data = os.environ.get("LOCALAPPDATA")
        if not local_app_data:
            raise SeedRefused("LOCALAPPDATA is not set")
        return Path(local_app_data)
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support"
    return Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")


def folders(data_folder):
    """The installed application's folder and the development folder, side by side."""
    return data_folder / INSTALLED_IDENTIFIER, data_folder / DEVELOPMENT_IDENTIFIER


def seed(installed, development, replace):
    """Copy the installed database into the development folder and detach the copy."""
    source = installed / DATABASE_FILENAME
    if not source.is_file():
        raise SeedRefused(f"no installed database at {source}")
    target = development / DATABASE_FILENAME
    if target.exists() and not replace:
        raise SeedRefused(f"{target} exists — pass --replace to overwrite it")

    development.mkdir(parents=True, exist_ok=True)
    remove_database(development)
    for suffix in COPIED_SUFFIXES:
        part = installed / f"{DATABASE_FILENAME}{suffix}"
        if part.is_file():
            shutil.copy2(part, development / part.name)

    try:
        detach(target)
    except (SeedRefused, sqlite3.Error):
        remove_database(development)
        raise
    return target


def remove_database(folder):
    """Delete a folder's database and its write-ahead files."""
    for suffix in DATABASE_SUFFIXES:
        (folder / f"{DATABASE_FILENAME}{suffix}").unlink(missing_ok=True)


def detach(database):
    """Remove the sync membership and disable the daily fetch, then check the copy is sound."""
    connection = sqlite3.connect(database)
    try:
        tables = {
            row[0]
            for row in connection.execute("SELECT name FROM sqlite_master WHERE type = 'table'")
        }
        if "sync_device" in tables:
            connection.execute("DELETE FROM sync_device")
        if "scheduled_fetch_configuration" in tables:
            connection.execute("UPDATE scheduled_fetch_configuration SET enabled = 0")
        connection.commit()
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        verdict = connection.execute("PRAGMA integrity_check").fetchone()[0]
    finally:
        connection.close()
    if verdict != "ok":
        raise SeedRefused(
            f"the copy failed its integrity check ({verdict}) — "
            "close the installed application and seed again"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--replace", action="store_true", help="overwrite an existing development database"
    )
    arguments = parser.parse_args()
    try:
        installed, development = folders(platform_data_folder())
        target = seed(installed, development, arguments.replace)
    except (SeedRefused, sqlite3.Error) as refusal:
        print(f"❌ {refusal}", file=sys.stderr)
        return 1
    print(f"✅ {target} seeded from {installed} (sync membership removed, daily fetch disabled)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
