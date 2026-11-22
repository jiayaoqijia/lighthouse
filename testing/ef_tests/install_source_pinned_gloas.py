#!/usr/bin/env python3
"""Admit only the reviewed source-generated Gloas fork vectors."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile

BUNDLE = Path(__file__).parent / "source-pinned" / "gloas-2ea9d96a"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verified_files(bundle):
    manifest = json.loads((bundle / "manifest.json").read_text())
    for entry in (manifest["source_archive"], manifest["vector_archive"]):
        if digest((bundle / entry["name"]).read_bytes()) != entry["sha256"]:
            raise ValueError("Archive digest mismatch: " + entry["name"])
    result = {}
    with tarfile.open(fileobj=io.BytesIO((bundle / manifest["vector_archive"]["name"]).read_bytes()), mode="r:gz") as archive:
        for member in archive.getmembers():
            path = PurePosixPath(member.name)
            parts = path.parts
            if not member.isfile() or member.name in result or len(parts) != 8 or parts[0] != "tests" or parts[1] not in ("minimal", "mainnet") or parts[2:6] != ("gloas", "fork", "fork", "pyspec_tests") or ".." in parts:
                raise ValueError("Unexpected or duplicate vector member: " + member.name)
            data = archive.extractfile(member).read()
            if digest(data) != manifest["files"].get(member.name):
                raise ValueError("Vector digest mismatch: " + member.name)
            result[member.name] = data
    if result.keys() != manifest["files"].keys():
        raise ValueError("Missing vector members")
    for preset, count in manifest["case_counts"].items():
        actual = sum(name.startswith(f"tests/{preset}/") and name.endswith("/post.ssz_snappy") for name in result)
        if actual != count or not actual:
            raise ValueError("Missing test cases for " + preset)
    return result


def install(bundle, output):
    files = verified_files(bundle)  # Validate everything before touching the fixture tree.
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".gloas-stage-", dir=output.parent) as temporary:
        stage = Path(temporary)
        for name, data in files.items():
            target = stage / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        replaced = []
        try:
            for preset in ("minimal", "mainnet"):
                target = output / "tests" / preset / "gloas" / "fork"
                target.parent.mkdir(parents=True, exist_ok=True)
                backup = stage / (preset + "-previous")
                if target.exists():
                    target.rename(backup)
                replaced.append((target, backup))
                (stage / "tests" / preset / "gloas" / "fork").rename(target)
        except BaseException:
            for target, backup in reversed(replaced):
                if target.exists():
                    shutil.rmtree(target)
                if backup.exists():
                    backup.rename(target)
            raise
    print(f"Installed {len(files)} independently generated, digest-verified Gloas fork files.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent / "consensus-spec-tests")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    if args.verify_only:
        print(f"Verified {len(verified_files(BUNDLE))} Gloas fork files.")
    else:
        install(BUNDLE, args.output)
