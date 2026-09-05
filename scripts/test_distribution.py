"""Test release archives and the Unix installer without network or user data."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

from package_release import ROOT, TARGETS, package


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="pomo-distribution-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "test-binary"
        self.binary.write_bytes(b"pomo release fixture\n")
        self.assets = self.root / "assets"

    def test_all_archives_have_only_binary_license_and_readme(self):
        for target in TARGETS:
            with self.subTest(target=target):
                archive = package(target, self.assets, self.binary)
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                self.assertEqual(archive.with_name(archive.name + ".sha256").read_text(), f"{digest}  {archive.name}\n")
                if archive.suffix == ".zip":
                    with zipfile.ZipFile(archive) as contents:
                        self.assertEqual(set(contents.namelist()), {"pomo.exe", "LICENSE", "README.md"})
                        self.assertEqual(contents.read("pomo.exe"), self.binary.read_bytes())
                else:
                    with tarfile.open(archive) as contents:
                        self.assertEqual(set(contents.getnames()), {"pomo", "LICENSE", "README.md"})
                        self.assertEqual(contents.getmember("pomo").mode, 0o755)
                        self.assertEqual(contents.extractfile("pomo").read(), self.binary.read_bytes())

    def test_missing_binary_and_unknown_target_fail(self):
        with self.assertRaises(FileNotFoundError):
            package(TARGETS[0], self.assets, self.root / "missing")
        with self.assertRaises(ValueError):
            package("unsupported", self.assets, self.binary)

    @unittest.skipIf(os.name == "nt", "Windows uses the PowerShell installer test")
    def test_installer_handles_platforms_pin_paths_and_failed_checksums(self):
        for target in TARGETS:
            package(target, self.assets, self.binary)
        sums = "".join(path.read_text() for path in sorted(self.assets.glob("*.sha256")))
        (self.assets / "SHA256SUMS").write_text(sums)
        mock_bin = self.root / "mock-bin"
        mock_bin.mkdir()
        # Use the existing Python executable; neither mock can contact the network.
        (mock_bin / "curl").write_text(
            "#!/bin/sh\nexec " + shell_quote(sys.executable) + " " + shell_quote(str(mock_bin / "download.py")) + ' "$@"\n'
        )
        (mock_bin / "download.py").write_text(
            "import os,sys,shutil\nfrom pathlib import Path\n"
            "args=sys.argv[1:]\nurl=next(a for a in args if a.startswith('https://github.com/'))\n"
            "assert url.startswith(os.environ['EXPECTED_BASE'])\n"
            "shutil.copyfile(Path(os.environ['ASSETS']) / url.rsplit('/',1)[1], args[args.index('--output')+1])\n"
        )
        (mock_bin / "uname").write_text('#!/bin/sh\ncase "$1" in -s) echo "$MOCK_OS";; -m) echo "$MOCK_ARCH";; esac\n')
        for name in ("curl", "uname"):
            (mock_bin / name).chmod(0o755)
        env = {**os.environ, "PATH": str(mock_bin) + os.pathsep + os.environ["PATH"],
               "ASSETS": str(self.assets), "POMO_VERSION": "v0.2.0", "TMPDIR": str(self.root),
               "EXPECTED_BASE": "https://github.com/AzAINN/Pomodoro/releases/download/v0.2.0/"}
        install = self.root / "directory with spaces" / "bin"
        env["POMO_INSTALL_DIR"] = str(install)
        for system, architecture in [("Darwin", "arm64"), ("Darwin", "x86_64"), ("Linux", "aarch64"), ("Linux", "x86_64")]:
            with self.subTest(platform=(system, architecture)):
                env.update(MOCK_OS=system, MOCK_ARCH=architecture)
                result = subprocess.run(["sh", str(ROOT / "install.sh")], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((install / "pomo").read_bytes(), self.binary.read_bytes())
                self.assertTrue(os.access(install / "pomo", os.X_OK))

        (install / "pomo").write_bytes(b"existing installation")
        wrong_hashes = "".join("0" * 64 + "  " + line.split()[1] + "\n" for line in sums.splitlines())
        for bad_sums in ["", wrong_hashes, sums + sums]:
            (self.assets / "SHA256SUMS").write_text(bad_sums)
            result = subprocess.run(["sh", str(ROOT / "install.sh")], env=env, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual((install / "pomo").read_bytes(), b"existing installation")
        self.assertFalse(list(self.root.glob("pomo-install.*")), "Installer left temporary files")
        env["POMO_INSTALL_DIR"] = "relative/path"
        self.assertNotEqual(subprocess.run(["sh", str(ROOT / "install.sh")], env=env, capture_output=True).returncode, 0)


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


if __name__ == "__main__":
    unittest.main(verbosity=2)
