"""Focused packaging tests: no game execution or access to a real save."""
from pathlib import Path
import contextlib
import hashlib
import io
import json
import sqlite3
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/github"))
sys.path.insert(0, str(ROOT / "scripts"))
from export_public import export, policy_reason
from setup_seed import create_seed


class PackagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.policy = json.loads((ROOT / "scripts/github/policy.json").read_text(encoding="utf8"))
        cls.temp_root = ROOT / "outputs/packaging-tests"
        cls.temp_root.mkdir(parents=True, exist_ok=True)

    def test_private_and_unlicensed_inputs_are_excluded(self):
        for path in [
            "runtime/switch/ragswitch-pre/ragnarok.db",
            "build/cargo-registry/cache", "MEMORIA-PROJETO.md",
            "provenance/fork-source-sha256.json",
            "vendor/classic-maps/data/izlude.gat",
            "vendor/translation-ro-br/System/itemInfo.lua",
            "vendor/mtp/mtp-tools/bin/tool.exe",
            "source/nostalre-launcher/client/switch/romfs/server/ragnarok.db",
            "source/game-data/data.grf", "docs/.env",
            "source/nostalre-launcher/client/docs/images/android-handheld.webp",
            "scripts/install_pre12.py", "scripts/port_renewal_pre2.py",
            "source/mesa-switch/src/gallium/targets/teflon/tests/models/test.tflite",
            "../outside/file", "/etc/passwd",
        ]:
            with self.subTest(path=path):
                self.assertIsNotNone(policy_reason(path, self.policy))

    def test_sources_notices_and_synthetic_fixtures_are_retained(self):
        for path in [
            "README.md", "config/seed.sql", "source/bevy-horizon/LICENSE-MIT",
            "source/nostalre-launcher/server/src/map/map.cpp",
            "source/nostalre-launcher/client/NOTICE",
            "source/nostalre-launcher/client/lib/formats/tests/fixtures/npcid_lua51.lub",
            "source/localization/npc-names-ptbr.json",
            "scripts/setup.ps1", "scripts/github/export_public.py",
        ]:
            with self.subTest(path=path):
                self.assertIsNone(policy_reason(path, self.policy))

    def test_seed_is_valid_empty_and_has_correct_identity(self):
        with tempfile.TemporaryDirectory(dir=self.temp_root) as tmp:
            path = Path(tmp) / "seed.db"
            with contextlib.redirect_stdout(io.StringIO()):
                create_seed(path)
            self.assertEqual(path.read_bytes()[18:20], bytes([1, 1]))
            with contextlib.closing(sqlite3.connect(path)) as db:
                self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
                self.assertEqual(db.execute('SELECT COUNT(*) FROM "char"').fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT edition FROM edition_identity").fetchone()[0], "ragswitch-prerenewal")
                self.assertEqual(db.execute("SELECT userid FROM login ORDER BY account_id").fetchall(), [("s1",), ("offline",)])
                self.assertEqual(db.execute("SELECT COUNT(*) FROM db_roulette").fetchone()[0], 42)

    def test_seed_never_overwrites_existing_file(self):
        with tempfile.TemporaryDirectory(dir=self.temp_root) as tmp:
            path = Path(tmp) / "existing.db"
            path.write_bytes(b"must stay unchanged")
            digest = hashlib.sha256(path.read_bytes()).digest()
            with self.assertRaises(FileExistsError):
                create_seed(path)
            self.assertEqual(hashlib.sha256(path.read_bytes()).digest(), digest)

    def test_export_refuses_existing_directory_and_parent(self):
        with tempfile.TemporaryDirectory(dir=self.temp_root) as tmp:
            sentinel = Path(tmp) / "sentinel"
            sentinel.write_text("preserve", encoding="utf8")
            with self.assertRaises(FileExistsError):
                export(Path(tmp))
            self.assertEqual(sentinel.read_text(encoding="utf8"), "preserve")
        with self.assertRaises(ValueError):
            export(ROOT.parent)


if __name__ == "__main__":
    unittest.main()
