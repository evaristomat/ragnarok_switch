"""Check project docs and public-export hygiene without running the game."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import subprocess
import sys
import urllib.parse
import os
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parents[2]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts/github"))
from export_public import policy_reason

DOCS = [
    "README.md", "docs/README.pt-BR.md", "CONTRIBUTING.md", "CHANGELOG.md", "NOTICE", "LICENSE",
    "docs/BUILDING.md", "docs/INSTALLATION.md", "docs/ARCHITECTURE.md",
    "docs/ASSETS.md", "docs/TESTING.md", "docs/LICENSING.md",
    "docs/PUBLISHING.md", "docs/STATUS.md", "docs/DISCLAIMER.md", "scripts/README.md",
]
TOKENS = re.compile(
    rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|"
    rb"\bgh[pousr]_[A-Za-z0-9]{36,}\b|"
    rb"\bgithub_pat_[A-Za-z0-9_]{50,}\b|"
    rb"\bAKIA[A-Z0-9]{16}\b"
)
LOCAL_PROFILE = re.compile(rb"(?i)\b[a-z]:[/\\]users[/\\]matheus(?:[/\\]|$)")


def check(public: bool) -> list[str]:
    errors = []
    for name in DOCS + ["edition.json", "config/seed.sql", "scripts/github/policy.json"]:
        if not (ROOT / name).is_file():
            errors.append("Missing file: " + name)
    for name in DOCS:
        path = ROOT / name
        if path.suffix != ".md" or not path.is_file():
            continue
        content = path.read_text(encoding="utf8")
        for href in re.findall(r"(?<!!)\[[^\]]+\]\(([^)]+)\)", content):
            href = href.strip("<>").split("#", 1)[0]
            if not href or urllib.parse.urlparse(href).scheme:
                continue
            if not (path.parent / urllib.parse.unquote(href)).is_file():
                errors.append(f"Broken link in {name}: {href}")
    edition = json.loads((ROOT / "edition.json").read_text(encoding="utf8"))
    if edition["version"] not in (ROOT / "README.md").read_text(encoding="utf8"):
        errors.append("README does not mention current edition version")
    # Validate the reviewed seed in memory, without touching the local seed/save.
    import sqlite3
    with sqlite3.connect(":memory:") as db:
        db.executescript((ROOT / "config/seed.sql").read_text(encoding="utf8"))
        if db.execute('SELECT count(*) FROM "char"').fetchone()[0]:
            errors.append("SQL seed contains characters")
        if db.execute("SELECT edition FROM edition_identity WHERE id=1").fetchone()[0] != edition["id"]:
            errors.append("SQL seed edition mismatch")
        if db.execute("SELECT count(*) FROM login").fetchone()[0] != 2:
            errors.append("SQL seed must contain only two synthetic accounts")
    if public:
        policy = json.loads((ROOT / "scripts/github/policy.json").read_text(encoding="utf8"))
        manifest_path = ROOT / "PUBLIC-SOURCE-MANIFEST.json"
        fingerprints = {}
        if manifest_path.exists():
            manifest = json.loads(manifest_path.read_text(encoding="utf8"))
            # Manifest fingerprints describe the initial export, not subsequent fork edits.
            if not (ROOT / ".git").exists():
                fingerprints = {entry["path"]: entry["sha256"] for entry in manifest["files"]}
        if (ROOT / ".git").exists():
            names = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode("utf8").rstrip("\0").split("\0")
        else:
            names = [Path(parent, name).relative_to(ROOT).as_posix()
                     for parent, _, files in os.walk(ROOT) for name in files]
        def inspect_file(name):
            issues = []
            if not name:
                return issues
            reason = policy_reason(name, policy)
            if reason:
                return [f"Excluded content present: {name} ({reason})"]
            path = ROOT / name
            if path.is_symlink():
                return ["Link in public export: " + name]
            size = path.stat().st_size
            if size > 100 * 1024 * 1024:
                return ["File exceeds GitHub 100 MiB limit: " + name]
            data = path.read_bytes()
            if name in fingerprints and hashlib.sha256(data).hexdigest() != fingerprints[name]:
                issues.append("Export hash mismatch: " + name)
            if size < 20 * 1024 * 1024:
                if TOKENS.search(data):
                    issues.append("Possible secret; inspect file without logging values: " + name)
                if LOCAL_PROFILE.search(data):
                    issues.append("Personal machine path: " + name)
            return issues
        print(f"Checking {len(names)} public files...", flush=True)
        with ThreadPoolExecutor(max_workers=8) as pool:
            for issues in pool.map(inspect_file, names):
                errors.extend(issues)
        missing = fingerprints.keys() - set(names)
        errors.extend("Missing exported file: " + name for name in missing)
        print("Publication permission pending: AzzyAI-derived defaults (docs/LICENSING.md).")
    return errors


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--public", action="store_true")
    args = parser.parse_args()
    problems = check(args.public)
    if problems:
        print("\n".join(problems))
        raise SystemExit(1)
    print("PASS: repository structure" + (" and public-file checks" if args.public else ""))
