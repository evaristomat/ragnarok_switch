"""Export selected current files to a NEW directory, without private Git history."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parents[2]


def policy_reason(name: str, policy: dict) -> str | None:
    p = PurePosixPath(name)
    if p.is_absolute() or ".." in p.parts or "\\" in name:
        return "invalid path"
    if p.parts[0] not in policy["allowed_roots"] and not name.startswith(policy["allowed_vendor_prefix"]):
        return "local-only root"
    if any(part in (".git", "__pycache__", "target", "save", "saves") for part in p.parts):
        return "generated or private directory"
    if any(name.startswith(prefix) for prefix in policy["excluded_prefixes"]):
        return "excluded source content"
    if name in policy["excluded_names"]:
        return "excluded individual file"
    if p.suffix.lower() in policy["excluded_suffixes"] or p.suffix.lower() == ".log":
        return "binary, database or local file"
    if p.name == ".env" or (p.name.startswith(".env.") and p.name != ".env.example"):
        return "environment configuration"
    if p.parts[0] == "scripts" and (
        p.name in policy["excluded_script_names"] or p.name.startswith("install_pre")
    ):
        return "local migration, translation pipeline or device installer"
    return None


def export(destination: Path, initialize_git: bool = False) -> dict:
    destination = destination.resolve()
    if destination == ROOT or ROOT.is_relative_to(destination):
        raise ValueError("Destination cannot be the source root or one of its parents")
    if destination.is_relative_to(ROOT) and not destination.is_relative_to(ROOT / "outputs"):
        raise ValueError("Inside this checkout, use outputs/ as destination")
    if destination.exists():
        raise FileExistsError(f"Destination already exists; preserved: {destination}")
    policy = json.loads((ROOT / "scripts/github/policy.json").read_text(encoding="utf8"))
    raw = subprocess.check_output(["git", "ls-files", "--cached", "-z"], cwd=ROOT)
    # New tooling/docs are eligible; untracked files in source/ are never swept
    # into a public export by accident.
    raw += subprocess.check_output(
        ["git", "ls-files", "--others", "--exclude-standard", "-z", "--",
         ".github", "campaign", "config", "docs", "scripts", "toolchain",
         ".editorconfig", ".gitattributes", ".gitignore", "README.md",
         "LICENSE", "NOTICE", "CONTRIBUTING.md", "CHANGELOG.md", "edition.json"],
        cwd=ROOT
    )
    candidates = sorted(set(raw.decode("utf8").rstrip("\0").split("\0")))
    selected = []
    excluded = {}
    for name in candidates:
        if not name or name == "PUBLIC-SOURCE-MANIFEST.json":
            continue
        reason = policy_reason(name, policy)
        if reason:
            excluded[reason] = excluded.get(reason, 0) + 1
            continue
        path = ROOT / name
        if not path.is_file():
            continue
        if path.is_symlink() or not path.resolve().is_relative_to(ROOT):
            raise ValueError(f"External link refused: {name}")
        selected.append(name)
    destination.mkdir(parents=True)
    def copy_file(name):
        path, target = ROOT / name, destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        data = path.read_bytes()
        target.write_bytes(data)
        if target.suffix == ".sh":
            target.chmod(0o755)
        return {"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    print(f"Exporting {len(selected)} selected files...", flush=True)
    with ThreadPoolExecutor(max_workers=8) as pool:
        files = list(pool.map(copy_file, selected))
    manifest = {
        "schema": 1,
        "edition": json.loads((ROOT / "edition.json").read_text(encoding="utf8")),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_working_tree_included": True,
        "private_history_included": False,
        "game_assets_included": False,
        "publication_approved": False,
        "publication_blockers": policy["known_publication_blockers"],
        "excluded_counts": excluded, "files": files,
    }
    (destination / "PUBLIC-SOURCE-MANIFEST.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf8"
    )
    subprocess.run([sys.executable, "-B", str(destination / "scripts/github/check_repository.py"), "--public"], cwd=destination, check=True)
    if initialize_git:
        subprocess.run(["git", "init", "-b", "main"], cwd=destination, check=True)
        subprocess.run(["git", "add", "--all"], cwd=destination, check=True)
        subprocess.run(
            ["git", "-c", "user.name=RagSWITCH maintainers",
             "-c", "user.email=maintainers@example.invalid",
             "-c", "commit.gpgsign=false", "commit", "-q", "-m",
             "Prepare independent Pre-Renewal source snapshot"],
            cwd=destination, check=True
        )
    print(json.dumps({"destination": str(destination), "files": len(files),
                      "bytes": sum(f["bytes"] for f in files),
                      "publication_approved": False}, ensure_ascii=False))
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--init-git", action="store_true")
    args = parser.parse_args()
    export(args.destination, args.init_git)
