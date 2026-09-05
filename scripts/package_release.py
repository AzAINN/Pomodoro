"""Create a release archive and its checksum from a built target binary."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
)


def package(target: str, output: Path, binary: Path | None = None) -> Path:
    if target not in TARGETS:
        raise ValueError(f"Unsupported release target: {target}")
    windows = target.endswith("windows-msvc")
    name = "pomo.exe" if windows else "pomo"
    binary = binary or ROOT / "target" / target / "release" / name
    if not binary.is_file():
        raise FileNotFoundError(f"Build the release binary first: {binary}")
    files = [(binary, name), (ROOT / "LICENSE", "LICENSE"), (ROOT / "README.md", "README.md")]
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"pomo-{target}.{'zip' if windows else 'tar.gz'}"
    if windows:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as result:
            for path, member in files:
                result.write(path, member)
    else:
        with tarfile.open(archive, "w:gz") as result:
            for path, member in files:
                info = result.gettarinfo(str(path), arcname=member)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mode = 0o755 if member == "pomo" else 0o644
                with path.open("rb") as content:
                    result.addfile(info, content)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--binary", type=Path, help="Override binary path for local packaging checks")
    args = parser.parse_args()
    print(package(args.target, args.output, args.binary))
