"""Create the embedded seed from reviewed SQL; never overwrite an existing DB."""
from pathlib import Path
import argparse
from contextlib import closing
import sqlite3
import tempfile

ROOT = Path(__file__).resolve().parent.parent
DEFAULT = ROOT / "source/nostalre-launcher/client/switch/romfs/server/ragnarok.db"


def create_seed(destination: Path) -> None:
    destination = destination.resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        raise FileExistsError(f"Database already exists; preserved: {destination}")
    with tempfile.TemporaryDirectory(prefix="seed-", dir=destination.parent) as tmp:
        pending = Path(tmp) / "ragnarok.db"
        with closing(sqlite3.connect(pending)) as db:
            db.executescript((ROOT / "config/seed.sql").read_text(encoding="utf8"))
            assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert db.execute('SELECT COUNT(*) FROM "char"').fetchone()[0] == 0
            assert db.execute("SELECT edition FROM edition_identity WHERE id=1").fetchone()[0] == "ragswitch-prerenewal"
            assert db.execute("PRAGMA journal_mode").fetchone()[0] == "delete"
        # Exclusive creation also protects against another process creating the target.
        try:
            with destination.open("xb") as output:
                output.write(pending.read_bytes())
        except FileExistsError:
            raise FileExistsError(f"Database appeared during setup; preserved: {destination}")
    print(f"Created synthetic offline seed: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT)
    args = parser.parse_args()
    create_seed(args.output)
