"""Exercise installer boundaries with temporary executable package fixtures."""

import contextlib
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("claude-codex")
MODULE = runpy.run_path(str(SCRIPT))


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="claude-codex-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.prefix = self.root / "prefix with spaces ' quoted"
        self.package = self.root / "package"
        self.target = MODULE["native_target"]()
        metadata = {
            "layoutVersion": 1,
            "version": "0.0.0",
            "target": self.target,
            "variant": "codex",
            "entrypoint": "bin/codex",
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        }
        for name in (
            "bin/codex",
            "bin/codex-code-mode-host",
            "codex-path/rg",
            "codex-resources/bwrap",
            "codex-resources/zsh/bin/zsh",
        ):
            path = self.package / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                "#!/usr/bin/env python3\nimport json, os, sys\nprint(json.dumps({'args': sys.argv[1:], 'home': os.environ.get('CODEX_HOME')}))\n"
            )
            path.chmod(0o755)
        (self.package / "codex-package.json").write_text(json.dumps(metadata))

    def invoke(self, *args, success=True):
        result = subprocess.run([str(SCRIPT), *args], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0 if success else 1, result.stderr)
        return result

    def install(self, *extra, success=True):
        return self.invoke(
            "install",
            "--prefix",
            str(self.prefix),
            "--package-dir",
            str(self.package),
            *extra,
            success=success,
        )

    def test_help_and_dry_run_do_not_write(self):
        self.invoke("install", "--invalid", "--help")
        self.install("--dry-run")
        self.assertFalse(self.prefix.exists())

    def test_complete_install_run_and_preserve_config_and_other_commands(self):
        other = self.prefix / "bin" / "codex"
        other.parent.mkdir(parents=True)
        other.write_text("unchanged normal codex")
        self.install()
        current = self.prefix / "share" / "claude-codex" / "current"
        self.assertTrue((current / "bin" / "codex-code-mode-host").is_file())
        arguments = ["--help", "a b", "$(no-execution)", "'quoted'"]
        env = dict(os.environ, CODEX_HOME=str(self.root / "account"))
        for command in (
            [str(self.prefix / "bin" / "claude-codex"), "--", *arguments],
            [str(SCRIPT), "run", "--prefix", str(self.prefix), "--", *arguments],
        ):
            result = subprocess.run(
                command, text=True, capture_output=True, env=env, check=True
            )
            self.assertEqual(
                json.loads(result.stdout),
                {
                    "args": ["--config", MODULE["STATUS_LINE"], *arguments],
                    "home": env["CODEX_HOME"],
                },
            )
        self.assertEqual(other.read_text(), "unchanged normal codex")
        self.assertIn(
            "CLI SHA-256:", self.invoke("status", "--prefix", str(self.prefix)).stdout
        )

    def test_update_needs_apply_and_retains_previous_package(self):
        self.install()
        current = self.prefix / "share" / "claude-codex" / "current"
        original = current.resolve()
        self.assertIn("Plan only", self.install().stdout)
        self.assertEqual(current.resolve(), original)
        self.install("--apply")
        self.assertNotEqual(current.resolve(), original)
        self.assertTrue(original.is_dir())

    def test_missing_host_rejected_without_install(self):
        (self.package / "bin" / "codex-code-mode-host").unlink()
        self.install(success=False)
        self.assertFalse(self.prefix.exists())

    def test_unmanaged_launcher_never_overwritten(self):
        launcher = self.prefix / "bin" / "claude-codex"
        launcher.parent.mkdir(parents=True)
        launcher.write_text("existing user launcher")
        self.install("--apply", success=False)
        self.assertEqual(launcher.read_text(), "existing user launcher")

    def test_build_invokes_complete_native_packager_and_records_success(self):
        function = MODULE["build"]
        args = MODULE["parser"]().parse_args(["build", "--jobs=2"])
        output = self.root / "builds"

        def fake_build(command, **kwargs):
            self.assertEqual(
                command[1], str(MODULE["ROOT"] / "scripts" / "build_codex_package.py")
            )
            self.assertEqual(command[command.index("--target") + 1], self.target)
            self.assertEqual(command[command.index("--cargo-profile") + 1], "dev-small")
            self.assertNotIn("--entrypoint-bin", command)
            self.assertNotIn("--code-mode-host-bin", command)
            self.assertEqual(kwargs["env"]["CARGO_BUILD_JOBS"], "2")
            destination = Path(command[command.index("--package-dir") + 1])
            import shutil

            shutil.copytree(self.package, destination)

        with (
            patch.dict(function.__globals__, {"BUILD_ROOT": output}),
            patch("subprocess.run", side_effect=fake_build),
            contextlib.redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(function(args, self.target), 0)
        package = Path(json.loads((output / "latest.json").read_text())["package"])
        self.assertTrue((package / "bin" / "codex-code-mode-host").is_file())


if __name__ == "__main__":
    unittest.main()
