#!/usr/bin/env python3
"""Install the unchanged current-source Gloas FCR vectors, replacing the affected stale beta.4 suites."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile

BUNDLE = Path(__file__).parent / "source-pinned" / "fcr-d2021d27"
REFERENCE = "d2021d2719235f0df4726a1d330502cb15aa8ea3"
FORK_COUNTS = {"gloas":196,"altair":18,"bellatrix":18,"capella":18,"deneb":18,"electra":31,"fulu":31}
SCOPES = ["tests/minimal/gloas/fast_confirmation"] + [
    f"tests/minimal/{fork}/fast_confirmation/{handler}"
    for fork in FORK_COUNTS if fork != "gloas"
    for handler in ("empty_slots", "is_one_confirmed")
]

def digest(data):
    return hashlib.sha256(data).hexdigest()

def verified_files(bundle):
    manifest = json.loads((bundle / "manifest.json").read_text())
    if manifest["reference"] != REFERENCE or manifest["case_counts"] != {"minimal": 330}:
        raise ValueError("Unexpected source or case inventory")
    for entry in (manifest["source_archive"], manifest["vector_archive"]):
        if Path(entry["name"]).name != entry["name"] or digest((bundle / entry["name"]).read_bytes()) != entry["sha256"]:
            raise ValueError("Archive digest mismatch")
    files = {}
    with tarfile.open(fileobj=io.BytesIO((bundle / manifest["vector_archive"]["name"]).read_bytes()), mode="r:gz") as archive:
        for member in archive:
            parts = PurePosixPath(member.name).parts
            if not member.isfile() or member.name in files or len(parts) != 8 or (parts[:2] != ("tests", "minimal") or parts[2] not in FORK_COUNTS or parts[3] != "fast_confirmation" or (parts[2] != "gloas" and parts[4] not in ("empty_slots", "is_one_confirmed"))) or ".." in parts:
                raise ValueError("Unexpected or duplicate vector member")
            data = archive.extractfile(member).read()
            if digest(data) != manifest["files"].get(member.name):
                raise ValueError("Vector digest mismatch")
            files[member.name] = data
    if files.keys() != manifest["files"].keys():
        raise ValueError("Missing vector members")
    counts = {fork:sum(name.split("/")[2] == fork and name.endswith("/manifest.yaml") for name in files) for fork in FORK_COUNTS}
    if counts != FORK_COUNTS:
        raise ValueError("Missing cases")
    return files

def install(bundle, output):
    files = verified_files(bundle)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".fcr-stage-", dir=output.parent) as temporary:
        stage = Path(temporary)
        for name, data in files.items():
            path = stage / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        replaced = []
        try:
            for index, scope in enumerate(SCOPES):
                target = output / scope
                target.parent.mkdir(parents=True, exist_ok=True)
                backup = stage / ("previous-"+str(index))
                if target.exists():
                    target.rename(backup)
                replaced.append((target, backup))
                (stage / scope).rename(target)
        except BaseException:
            for target, backup in reversed(replaced):
                if target.exists():
                    shutil.rmtree(target)
                if backup.exists():
                    backup.rename(target)
            raise
    print(f"Installed 330 source-pinned FCR cases ({len(files)} files); Gloas196 plus historical compatibility134; synthetic signatures, real public keys.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent / "consensus-spec-tests")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    if args.verify_only:
        print(f"Verified {len(verified_files(BUNDLE))} source-pinned FCR files.")
    else:
        install(BUNDLE, args.output)
