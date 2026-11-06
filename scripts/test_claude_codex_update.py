"""Release updater boundaries exercised without network or the real install."""

import hashlib
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.machinery.SourceFileLoader(
    "release_updater", str(Path(__file__).with_name("claude-codex-update"))
)
spec = importlib.util.spec_from_loader(loader.name, loader)
u = importlib.util.module_from_spec(spec)
loader.exec_module(u)


class ReleaseUpdates(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.base = self.root / "install"
        self.old = self.base / "packages/old"
        self.old.mkdir(parents=True)
        (self.old / "codex-package.json").write_text(json.dumps({"version": "0.165.0"}))
        (self.base / "current").symlink_to(self.old)
        self.package = self.root / "package"
        self.package.mkdir()
        self.target = "aarch64-apple-darwin"
        (self.package / "codex-package.json").write_text(
            json.dumps(
                dict(
                    version="0.166.0",
                    target=self.target,
                    variant="codex",
                    entrypoint="bin/codex",
                )
            )
        )
        for name in [
            "bin/codex",
            "bin/codex-code-mode-host",
            "bin/codex-exec-server",
            "codex-path/rg",
        ]:
            path = self.package / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/bin/sh\necho codex-cli 0.166.0\n")
            path.chmod(0o755)
        self.archive = self.root / f"claude-codex-{self.target}.tar.gz"
        self.checksum = self.root / (self.archive.name + ".sha256")
        self.release = {
            "tag_name": "ui-v0.166.0-1",
            "assets": [
                {
                    "name": path.name,
                    "browser_download_url": f"https://github.com/{u.REPOSITORY}/releases/download/ui-v0.166.0-1/{path.name}",
                }
                for path in [self.archive, self.checksum]
            ],
        }
        self.pack()
        downloader = patch.object(
            u,
            "download",
            side_effect=lambda url, path: shutil.copyfile(
                self.root / url.rsplit("/", 1)[1], path
            ),
        )
        downloader.start()
        self.addCleanup(downloader.stop)

    def pack(self):
        with tarfile.open(self.archive, "w:gz") as bundle:
            for path in self.package.rglob("*"):
                bundle.add(
                    path, arcname=str(path.relative_to(self.package)), recursive=False
                )
        self.hash()

    def hash(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksum.write_text(f"{digest}  {self.archive.name}\n")

    def test_complete_package_switches_and_preserves_previous(self):
        self.assertEqual(
            u.install_release(self.base, self.release, self.target), "0.166.0"
        )
        self.assertNotEqual((self.base / "current").resolve(), self.old)
        self.assertTrue(self.old.exists())
        self.assertEqual(
            u.read_json(self.base / "current/claude-codex-release.json")["tag"],
            "ui-v0.166.0-1",
        )

    def test_bad_checksum_keeps_current(self):
        self.archive.write_bytes(b"corrupt")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            u.install_release(self.base, self.release, self.target)
        self.assertEqual((self.base / "current").resolve(), self.old)

    def test_missing_host_keeps_current(self):
        (self.package / "bin/codex-code-mode-host").unlink()
        self.pack()
        with self.assertRaisesRegex(RuntimeError, "missing a valid"):
            u.install_release(self.base, self.release, self.target)
        self.assertEqual((self.base / "current").resolve(), self.old)

    def test_wrong_binary_version_keeps_current(self):
        (self.package / "bin/codex").write_text("#!/bin/sh\necho codex-cli 0.153.4\n")
        self.pack()
        with self.assertRaisesRegex(RuntimeError, "unexpected version"):
            u.install_release(self.base, self.release, self.target)
        self.assertEqual((self.base / "current").resolve(), self.old)

    def test_archive_traversal_is_rejected(self):
        with tarfile.open(self.archive, "w:gz") as bundle:
            member = tarfile.TarInfo("../../../../outside")
            member.size = 3
            bundle.addfile(member, io.BytesIO(b"bad"))
        self.hash()
        with self.assertRaises(tarfile.TarError):
            u.install_release(self.base, self.release, self.target)
        self.assertEqual((self.base / "current").resolve(), self.old)
        self.assertFalse((self.root / "outside").exists())

    def test_unexpected_asset_origin_is_rejected(self):
        self.release["assets"][0]["browser_download_url"] = (
            "https://example.org/fake.tar.gz"
        )
        with self.assertRaisesRegex(RuntimeError, "outside the expected"):
            u.install_release(self.base, self.release, self.target)

    def test_offline_scheduled_failure_is_recorded(self):
        with patch.object(u, "latest_release", side_effect=OSError("offline")):
            self.assertEqual(u.update(self.base, False, True), 0)
        self.assertEqual(
            u.read_json(self.base / "update-status.json")["result"], "failed"
        )
        self.assertEqual((self.base / "current").resolve(), self.old)

    def test_up_to_date_skips_download_and_dry_run_does_not_write(self):
        u.write_json(self.old / "claude-codex-release.json", {"tag": "ui-v0.166.0-1"})
        with (
            patch.object(u, "latest_release", return_value=self.release),
            patch.object(
                u, "download", side_effect=AssertionError("unexpected download")
            ),
        ):
            self.assertEqual(u.update(self.base, False, False), 0)
        missing = self.root / "missing"
        self.assertEqual(u.update(missing, True, False), 0)
        self.assertFalse(missing.exists())


if __name__ == "__main__":
    unittest.main()
