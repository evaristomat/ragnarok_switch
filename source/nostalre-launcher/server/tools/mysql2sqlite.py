#!/usr/bin/env python3
"""Convert rAthena's MySQL schema files (sql-files/*.sql) to SQLite.

Used by the ENABLE_SQLITE build. Usage:
    mysql2sqlite.py sql-files/main.sql sql-files/web.sql > ragnarok.sql
    sqlite3 ragnarok.db < ragnarok.sql

MySQL behaviour emulated:
- string columns get COLLATE NOCASE (MySQL's default collations are case-insensitive)
- NOT NULL columns without a default get '' / 0 (MySQL non-strict mode)
- AUTO_INCREMENT=N table option seeds sqlite_sequence
- strict mode (MariaDB's default): CHECK constraints reject strings longer than the column,
  integers outside the type's range and values outside an ENUM, like MySQL does
"""

import re
import sys

INT_TYPES = {"tinyint", "smallint", "mediumint", "int", "integer", "bigint"}
REAL_TYPES = {"float", "double", "decimal", "real"}
BLOB_TYPES = {"blob", "tinyblob", "mediumblob", "longblob", "binary", "varbinary"}
NOCASE_TYPES = {"char", "varchar", "enum", "set", "text", "tinytext", "mediumtext", "longtext"}
DATETIME_TYPES = {"datetime", "timestamp", "date", "time"}
# value ranges MySQL enforces in strict mode, (signed, unsigned)
INT_RANGES = {
    "tinyint": ((-2**7, 2**7 - 1), (0, 2**8 - 1)),
    "smallint": ((-2**15, 2**15 - 1), (0, 2**16 - 1)),
    "mediumint": ((-2**23, 2**23 - 1), (0, 2**24 - 1)),
    "int": ((-2**31, 2**31 - 1), (0, 2**32 - 1)),
    "integer": ((-2**31, 2**31 - 1), (0, 2**32 - 1)),
    "bigint": ((-2**63, 2**63 - 1), (0, None)),  # SQLite integers stop at 2^63 - 1
}


def split_top_level(text, sep):
    """Split on sep outside quotes, backticks and parentheses."""
    parts, buf, depth, quote = [], [], 0, None
    for ch in text:
        if quote:
            buf.append(ch)
            if ch == quote:
                quote = None
            continue
        if ch in "'`\"":
            quote = ch
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        elif ch == sep and depth == 0:
            parts.append("".join(buf))
            buf = []
            continue
        buf.append(ch)
    parts.append("".join(buf))
    return [p.strip() for p in parts if p.strip()]


def strip_comments(sql):
    lines = []
    for line in sql.splitlines():
        stripped = line.strip()
        if stripped.startswith("#") or stripped.startswith("--"):
            continue
        # trailing "-- comment" after a statement
        lines.append(re.sub(r"\s--(\s.*)?$", "", line))
    return re.sub(r"/\*.*?\*/", "", "\n".join(lines), flags=re.S)


def column_names(cols):
    return [c.strip().strip("`") for c in split_top_level(cols, ",")]


def convert_column(item):
    """Returns (name, sqlite definition, is_auto_increment)."""
    m = re.match(r"`(\w+)`\s+(\w+)(\s*\([^)]*\))?(.*)$", item, re.S | re.I)
    if not m:
        raise ValueError(f"unparsed column: {item}")
    name, mtype, size, rest = m.group(1), m.group(2).lower(), m.group(3), m.group(4)
    check = strict_check(name, mtype, (size or "").strip(), re.search(r"\bunsigned\b", rest, re.I) is not None)

    if mtype in INT_TYPES:
        stype, implicit = "INTEGER", "0"
    elif mtype in REAL_TYPES:
        stype, implicit = "REAL", "0"
    elif mtype in BLOB_TYPES:
        stype, implicit = "BLOB", "''"
    elif mtype in NOCASE_TYPES:
        stype, implicit = "TEXT COLLATE NOCASE", "''"
    elif mtype in DATETIME_TYPES:
        stype, implicit = "TEXT", "NULL"
    else:
        raise ValueError(f"unknown type {mtype} in: {item}")

    # "varchar(n) binary" is a case-sensitive string column
    if stype.startswith("TEXT COLLATE") and re.search(r"\bbinary\b", rest, re.I):
        stype = "TEXT"
        rest = re.sub(r"\bbinary\b", "", rest, flags=re.I)

    auto_inc = re.search(r"\bauto_increment\b", rest, re.I) is not None
    rest = re.sub(r"\b(unsigned|zerofill|auto_increment)\b", "", rest, flags=re.I)
    rest = re.sub(r"\bON\s+UPDATE\s+CURRENT_TIMESTAMP\b", "", rest, flags=re.I)
    rest = re.sub(r"\b(CHARACTER\s+SET|CHARSET|COLLATE)\s+\w+", "", rest, flags=re.I)
    rest = re.sub(r"\bCOMMENT\s+'(?:[^']|'')*'", "", rest, flags=re.I)
    rest = re.sub(r"\bDEFAULT\s+CURRENT_TIMESTAMP\b", "DEFAULT (datetime('now','localtime'))", rest, flags=re.I)
    rest = " ".join(rest.split())

    # MySQL's implicit default of a NOT NULL ENUM is its first value
    if mtype == "enum" and size:
        implicit = re.match(r"\(\s*('(?:[^']|'')*')", size).group(1)

    not_null = re.search(r"\bNOT\s+NULL\b", rest, re.I) is not None
    if not_null and not auto_inc and not re.search(r"\bDEFAULT\b", rest, re.I) and implicit != "NULL":
        rest += f" DEFAULT {implicit}"

    if check:
        rest += f" CHECK ({check})"
    return name, f"`{name}` {stype} {rest}".rstrip(), auto_inc


def strict_check(name, mtype, size, unsigned):
    """CHECK expression for what MySQL's strict mode rejects in this column, or None."""
    col = f"`{name}`"
    if mtype in INT_RANGES:
        low, high = INT_RANGES[mtype][1 if unsigned else 0]
        return f"{col} >= {low}" if high is None else f"{col} BETWEEN {low} AND {high}"
    if mtype in ("char", "varchar") and size:
        return f"length({col}) <= {int(size.strip('()'))}"
    if mtype == "enum":
        return f"{col} IN {size}"
    return None


def convert_create(stmt, out):
    m = re.match(r"CREATE\s+TABLE\s+(IF\s+NOT\s+EXISTS\s+)?`(\w+)`\s*\((.*)\)\s*(.*)$", stmt, re.S | re.I)
    if not m:
        raise ValueError(f"unparsed CREATE: {stmt[:80]}")
    table, body, options = m.group(2), m.group(3), m.group(4)

    columns, constraints, indexes = [], [], []
    primary, uniques, auto_col = None, [], None

    for item in split_top_level(body, ","):
        upper = item.upper()
        if item.startswith("`"):
            name, definition, auto_inc = convert_column(item)
            columns.append([name, definition])
            if auto_inc:
                auto_col = name
        elif upper.startswith("PRIMARY KEY"):
            primary = column_names(re.search(r"\((.*)\)", item, re.S).group(1))
        elif upper.startswith("UNIQUE"):
            uniques.append(column_names(re.search(r"\((.*)\)", item, re.S).group(1)))
        elif upper.startswith(("KEY", "INDEX")):
            km = re.match(r"(?:KEY|INDEX)\s*(?:`?(\w+)`?)?\s*\((.*)\)", item, re.S | re.I)
            cols = column_names(km.group(2))
            indexes.append((km.group(1) or "_".join(cols), cols))
        elif upper.startswith(("FOREIGN KEY", "CONSTRAINT")):
            constraints.append(" ".join(item.split()))
        else:
            raise ValueError(f"unknown table item in {table}: {item}")

    # SQLite only auto-increments a lone INTEGER PRIMARY KEY
    rowid_col = None
    if auto_col and (primary == [auto_col] or [auto_col] in uniques):
        rowid_col = auto_col
        if primary and primary != [auto_col]:
            uniques.append(primary)
        primary = None
        uniques = [u for u in uniques if u != [auto_col]]

    lines = []
    for name, definition in columns:
        if name == rowid_col:
            definition = re.sub(r"\bINTEGER\b", "INTEGER PRIMARY KEY AUTOINCREMENT", definition, count=1)
            definition = re.sub(r"\s*\bDEFAULT\s+\S+", "", definition)
        lines.append(definition)
    if primary:
        lines.append("PRIMARY KEY (" + ", ".join(f"`{c}`" for c in primary) + ")")
    for u in uniques:
        lines.append("UNIQUE (" + ", ".join(f"`{c}`" for c in u) + ")")
    lines.extend(constraints)

    out.append(f"CREATE TABLE IF NOT EXISTS `{table}` (\n  " + ",\n  ".join(lines) + "\n);")
    for name, cols in indexes:
        col_list = ", ".join(f"`{c}`" for c in cols)
        out.append(f"CREATE INDEX IF NOT EXISTS `{table}_{name}` ON `{table}` ({col_list});")

    seed = re.search(r"AUTO_INCREMENT\s*=\s*(\d+)", options, re.I)
    if rowid_col and seed and int(seed.group(1)) > 1:
        start = int(seed.group(1)) - 1
        out.append(
            f"INSERT INTO sqlite_sequence (name, seq) SELECT '{table}', {start} "
            f"WHERE NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = '{table}');"
        )


def convert(sql, out):
    for stmt in split_top_level(strip_comments(sql), ";"):
        head = stmt.split(None, 2)[:2]
        keyword = " ".join(head).upper()
        if keyword.startswith("CREATE TABLE"):
            convert_create(stmt, out)
        elif keyword.startswith(("INSERT", "REPLACE", "DROP TABLE", "DELETE", "UPDATE")):
            out.append(stmt + ";")
        elif keyword.startswith(("SET", "LOCK", "UNLOCK")):
            continue
        else:
            raise ValueError(f"unsupported statement: {stmt[:80]}")


def main():
    out = ["PRAGMA journal_mode=WAL;", "BEGIN;"]
    for path in sys.argv[1:]:
        with open(path, encoding="utf-8") as f:
            convert(f.read(), out)
    out.append("COMMIT;")
    print("\n".join(out))


if __name__ == "__main__":
    main()
