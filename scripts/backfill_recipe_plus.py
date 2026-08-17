#!/usr/bin/env python3
"""
Backfill recipe PLUs into description so the main recipe list/detail show badges.

Sources (priority order):
  1. Existing description PLU (leave alone)
  2. PLU codes embedded in recipe steps (ChefTec pack-out lines)
  3. Name match against plu-reference.json (exact + bare name)

Writes:
  - Prefixed description bit: "PLU 30526 · …" (parseable by the SPA)
  - Optional: strip pure-PLU noise steps after promoting to description

Usage:
  python3 scripts/backfill_recipe_plus.py --dry-run ~/.local/share/larder/work.db
  python3 scripts/backfill_recipe_plus.py ~/.local/share/larder/work.db
  python3 scripts/backfill_recipe_plus.py --bundle data/lexco-source/clean/kitchen-bundle.json
  python3 scripts/backfill_recipe_plus.py --db work.db --bundle kitchen-bundle.json
"""
from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REF = ROOT / "server" / "src" / "static" / "plu-reference.json"

# "Use the plu tag" / "plump" are not codes
NOISE_PLU_WORD = re.compile(r"\bplu\b", re.I)

# Full / half sandwich patterns (prefer Full first in display).
# Note: avoid re.X — bare # inside char classes is treated as a comment.
FULL_HALF_RE = re.compile(
    r"PLU\s*[:#]?\s*"
    r"(?:"
    r"(?:Full\s*[:#]?\s*(?P<full1>\d{3,6})\s*[,;/]?\s*Half\s*[:#]?\s*(?P<half1>\d{3,6}))"
    r"|"
    r"(?:Half\s*[:#]?\s*(?P<half2>\d{3,6})\s*[,;/]?\s*Full\s*[:#]?\s*(?P<full2>\d{3,6}))"
    r"|"
    r"(?:(?P<full3>\d{3,6})\s*[-–]?\s*full\s+(?P<half3>\d{3,6})\s*[-–]?\s*half)"
    r")",
    re.I,
)

# PLU with size note: "PLU (family size) 31403", "PLU (16 oz.) 30751"
SIZED_PLU_RE = re.compile(
    r"PLU\s*\((?P<label>[^)]+)\)\s*[:#]?\s*(?P<code>\d{3,6})",
    re.I,
)

# Plain: "PLU: 30612", "PLU 30172", "PLU# 033760", "PLU:24033", "…PLU: 30197"
PLAIN_PLU_RE = re.compile(
    r"PLU\s*[:#]?\s*(?P<code>\d{3,6})\b",
    re.I,
)

# Description already has a leading/segment PLU bit
DESC_PLU_RE = re.compile(
    r"(?:^|\s*·\s*)PLU\s+.+?(?=\s*·\s*|$)",
    re.I,
)


def normalize_code(code: str) -> str:
    """Strip leading zeros only when still ≥3 digits (033760 → 33760)."""
    s = str(code).strip()
    if not s.isdigit():
        return s
    stripped = s.lstrip("0") or "0"
    # keep 3–6 digit store PLUs; if stripping zeros leaves <3, keep original
    if 3 <= len(stripped) <= 6:
        return stripped
    return s


def extract_plu_from_text(text: str) -> str | None:
    """Return display PLU string (without leading 'PLU ') or None."""
    if not text:
        return None
    t = text.strip()
    # Ignore prose that only mentions the word "plu" without a code/structure
    # (e.g. "Use the plu tag to fold…", "plump up"). Allow "PLU Full:", "PLU: 1…".
    if "plu" in t.lower() and not re.search(
        r"plu\s*([:#(\d]|full|half)", t, re.I
    ):
        return None

    m = FULL_HALF_RE.search(t)
    if m:
        full = normalize_code(
            m.group("full1") or m.group("full2") or m.group("full3") or ""
        )
        half = normalize_code(
            m.group("half1") or m.group("half2") or m.group("half3") or ""
        )
        if full and half:
            return f"full {full} / half {half}"

    m = SIZED_PLU_RE.search(t)
    if m:
        label = re.sub(r"\s+", " ", m.group("label").strip())
        code = normalize_code(m.group("code"))
        return f"{code} ({label})"

    m = PLAIN_PLU_RE.search(t)
    if m:
        return normalize_code(m.group("code"))

    return None


def is_valid_plu_display(plu: str | None) -> bool:
    """Accept store codes and full/half sandwich forms; reject garbage like 'thai'."""
    if not plu:
        return False
    p = str(plu).strip()
    # full 30913 / half 30358
    if re.fullmatch(
        r"full\s+\d{3,6}\s*/\s*half\s+\d{3,6}", p, re.I
    ):
        return True
    # 31403 (family size)
    if re.fullmatch(r"\d{3,6}\s*\([^)]{1,40}\)", p):
        return True
    # bare 3–6 digit store PLU
    if re.fullmatch(r"\d{3,6}", p):
        return True
    return False


def description_has_plu(desc: str | None) -> bool:
    """True when description already has a *valid* PLU segment."""
    if not desc:
        return False
    d = str(desc).strip()
    # "PLU 30526 · …" or "PLU: 30526 · …" or leading freeform
    for bit in re.split(r"\s*·\s*", d):
        b = bit.strip()
        m = re.match(r"^PLU\s*[:#]?\s*(.+)$", b, re.I)
        if m and is_valid_plu_display(m.group(1).strip()):
            return True
        # bit may be "PLU: 30553" without space after colon handled above
    extracted = extract_plu_from_text(d)
    return is_valid_plu_display(extracted)


def strip_plu_segments(desc: str) -> str:
    """Remove existing PLU bits (valid or junk) from a description."""
    d = str(desc or "").strip()
    if not d:
        return ""
    parts = []
    for bit in re.split(r"\s*·\s*", d):
        b = bit.strip()
        if not b:
            continue
        if re.match(r"^PLU\b", b, re.I):
            continue
        # freeform with embedded PLU: 12345 — leave non-PLU prose
        if extract_plu_from_text(b) and re.match(r"^PLU\b", b, re.I):
            continue
        parts.append(b)
    return " · ".join(parts)


def inject_plu_into_description(desc: str | None, plu_display: str) -> str:
    """Prepend 'PLU … ·' to description; replace any existing PLU bit."""
    if not is_valid_plu_display(plu_display):
        return (desc or "").strip()
    bit = f"PLU {plu_display}"
    rest = strip_plu_segments(desc or "")
    if not rest:
        return bit
    return f"{bit} · {rest}"

def load_plu_by_name(path: Path) -> dict[str, str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    items = data.get("items") if isinstance(data, dict) else data
    out: dict[str, str] = {}
    for it in items or []:
        name = (it.get("name") or "").strip()
        plu = normalize_code(str(it.get("plu") or "").strip())
        if not name or not plu or not plu.isdigit():
            continue
        key = name.casefold()
        # first wins (reference already priority-sorted by merge script)
        out.setdefault(key, plu)
    return out


def name_lookup(name: str, plu_by_name: dict[str, str]) -> str | None:
    full = name.casefold().strip()
    if full in plu_by_name:
        return plu_by_name[full]
    bare = re.sub(r"\s*\([^)]*\)\s*$", "", full).strip()
    if bare and bare in plu_by_name:
        return plu_by_name[bare]
    # ChefTec "Pizza--Margherita" → try after --
    if "--" in full:
        tail = full.split("--", 1)[1].strip()
        if tail in plu_by_name:
            return plu_by_name[tail]
    return None


def resolve_plu(
    name: str,
    description: str | None,
    step_texts: list[str],
    plu_by_name: dict[str, str],
) -> tuple[str | None, str]:
    """Return (plu_display, source) where source is desc|step|name|none.

    Invalid description PLUs (e.g. ChefTec junk "PLU thai") are ignored so
    step/name can replace them.
    """
    if description:
        # ChefTec-style bits: "PLU 30526" / "PLU full … / half …"
        for bit in re.split(r"\s*·\s*", description):
            m = re.match(r"^PLU\s*[:#]?\s*(.+)$", bit.strip(), re.I)
            if m:
                candidate = m.group(1).strip()
                # strip a doubled "PLU:" prefix if present
                candidate = re.sub(r"^PLU\s*[:#]?\s*", "", candidate, flags=re.I).strip()
                if is_valid_plu_display(candidate):
                    return candidate, "desc"
                # try extracting digits from junk bit
                extracted = extract_plu_from_text(bit)
                if is_valid_plu_display(extracted):
                    return extracted, "desc"
        from_desc = extract_plu_from_text(description)
        if is_valid_plu_display(from_desc):
            return from_desc, "desc"

    for st in step_texts:
        p = extract_plu_from_text(st)
        if is_valid_plu_display(p):
            return p, "step"

    from_name = name_lookup(name, plu_by_name)
    if is_valid_plu_display(from_name):
        return from_name, "name"

    return None, "none"


def is_pure_plu_step(text: str) -> bool:
    """Step that is only a PLU line (safe to drop after promote)."""
    t = (text or "").strip()
    if not t:
        return False
    if extract_plu_from_text(t) is None:
        return False
    # strip the PLU bits; if almost nothing left, pure
    cleaned = FULL_HALF_RE.sub("", t)
    cleaned = SIZED_PLU_RE.sub("", cleaned)
    cleaned = PLAIN_PLU_RE.sub("", cleaned)
    cleaned = re.sub(r"^PLU\s*[:#]?\s*$", "", cleaned, flags=re.I)
    cleaned = re.sub(r"[^\w]+", " ", cleaned).strip()
    # leftover noise words
    if not cleaned or cleaned.lower() in {"full", "half", "all", "weather", "allergens", "wheat"}:
        return True
    # "ALLERGENS: WHEATALL WEATHERPLU: 30197" — keep if substantial allergen info
    if len(cleaned) > 24:
        return False
    if re.search(r"allergen", t, re.I) and len(cleaned) > 8:
        return False
    return len(cleaned) < 12


def backfill_db(
    db_path: Path,
    plu_by_name: dict[str, str],
    *,
    dry_run: bool,
    drop_plu_steps: bool,
) -> dict[str, int]:
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    stats = {
        "total": 0,
        "already": 0,
        "updated_step": 0,
        "updated_name": 0,
        "unchanged": 0,
        "steps_dropped": 0,
    }

    recipes = conn.execute(
        "SELECT id, name, description FROM recipes ORDER BY name"
    ).fetchall()
    stats["total"] = len(recipes)

    for r in recipes:
        rid = r["id"]
        name = r["name"] or ""
        desc = r["description"]
        steps = conn.execute(
            "SELECT id, instruction FROM recipe_steps WHERE recipe_id = ? ORDER BY position, id",
            (rid,),
        ).fetchall()
        step_texts = [s["instruction"] or "" for s in steps]

        plu, source = resolve_plu(name, desc, step_texts, plu_by_name)
        if not plu:
            stats["unchanged"] += 1
            continue

        new_desc = inject_plu_into_description(desc, plu)
        if new_desc == (desc or "").strip():
            stats["already"] += 1
            continue

        if not dry_run:
            conn.execute(
                "UPDATE recipes SET description = ?, updated_at = datetime('now') WHERE id = ?",
                (new_desc, rid),
            )
            if drop_plu_steps:
                for s in steps:
                    if is_pure_plu_step(s["instruction"] or ""):
                        conn.execute("DELETE FROM recipe_steps WHERE id = ?", (s["id"],))
                        stats["steps_dropped"] += 1

        if source == "desc":
            # reformatted existing (e.g. PLU: 123 → PLU 123)
            stats["already"] += 1
            # count as update if text changed — already handled; use updated_step bucket for normalize
            stats["updated_step"] += 1
            stats["already"] -= 1
        elif source == "step":
            stats["updated_step"] += 1
        else:
            stats["updated_name"] += 1
    if not dry_run:
        conn.commit()
        # refresh FTS if present
        try:
            conn.execute(
                """
                INSERT INTO recipe_fts(recipe_fts) VALUES('rebuild')
                """
            )
            conn.commit()
        except sqlite3.Error:
            # table may not use that rebuild form — try row-wise update of name/desc
            try:
                conn.execute("DELETE FROM recipe_fts")
                conn.execute(
                    """
                    INSERT INTO recipe_fts(recipe_id, name, description, ingredients)
                    SELECT r.id, r.name, coalesce(r.description, ''),
                           coalesce((
                             SELECT group_concat(ri.display, ' ')
                             FROM recipe_ingredients ri WHERE ri.recipe_id = r.id
                           ), '')
                    FROM recipes r
                    """
                )
                conn.commit()
            except sqlite3.Error:
                pass
    conn.close()
    return stats


def backfill_bundle(
    bundle_path: Path,
    plu_by_name: dict[str, str],
    *,
    dry_run: bool,
    drop_plu_steps: bool,
) -> dict[str, int]:
    data = json.loads(bundle_path.read_text(encoding="utf-8"))
    recipes = data.get("recipes") or []
    stats = {
        "total": len(recipes),
        "already": 0,
        "updated_step": 0,
        "updated_name": 0,
        "unchanged": 0,
        "steps_dropped": 0,
    }
    changed = False

    for r in recipes:
        name = r.get("name") or ""
        desc = r.get("description")
        steps = r.get("steps") or []
        step_texts = [
            (s.get("instruction") if isinstance(s, dict) else str(s)) or ""
            for s in steps
        ]

        plu, source = resolve_plu(name, desc, step_texts, plu_by_name)
        if not plu:
            stats["unchanged"] += 1
            continue

        new_desc = inject_plu_into_description(desc, plu)
        if new_desc == (desc or "").strip():
            stats["already"] += 1
            continue

        if not dry_run:
            r["description"] = new_desc
            changed = True
            if drop_plu_steps and isinstance(steps, list):
                kept = []
                for s in steps:
                    instr = (s.get("instruction") if isinstance(s, dict) else str(s)) or ""
                    if is_pure_plu_step(instr):
                        stats["steps_dropped"] += 1
                        continue
                    kept.append(s)
                # re-number positions
                for i, s in enumerate(kept, start=1):
                    if isinstance(s, dict):
                        s["position"] = i
                r["steps"] = kept

        if source == "desc":
            stats["updated_step"] += 1  # normalized form
        elif source == "step":
            stats["updated_step"] += 1
        else:
            stats["updated_name"] += 1

    if changed and not dry_run:
        bundle_path.write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    return stats


def print_stats(label: str, stats: dict[str, int]) -> None:
    print(f"\n{label}")
    print(f"  total recipes:     {stats['total']}")
    print(f"  already had PLU:   {stats['already']}")
    print(f"  filled from step:  {stats['updated_step']}")
    print(f"  filled from name:  {stats['updated_name']}")
    print(f"  still no PLU:      {stats['unchanged']}")
    if stats.get("steps_dropped"):
        print(f"  pure PLU steps dropped: {stats['steps_dropped']}")
    filled = stats["already"] + stats["updated_step"] + stats["updated_name"]
    if stats["total"]:
        print(f"  coverage:          {filled}/{stats['total']} ({100 * filled / stats['total']:.1f}%)")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "db",
        nargs="?",
        type=Path,
        help="SQLite DB path (e.g. ~/.local/share/larder/work.db)",
    )
    ap.add_argument(
        "--bundle",
        type=Path,
        help="kitchen-bundle.json (or any Larder recipe bundle) to update",
    )
    ap.add_argument(
        "--ref",
        type=Path,
        default=DEFAULT_REF,
        help="plu-reference.json path",
    )
    ap.add_argument("--dry-run", action="store_true", help="Report only, no writes")
    ap.add_argument(
        "--drop-plu-steps",
        action="store_true",
        help="After promoting PLU to description, delete pure-PLU steps",
    )
    args = ap.parse_args()

    if not args.db and not args.bundle:
        ap.error("Provide a DB path and/or --bundle")

    if not args.ref.is_file():
        print(f"error: PLU reference not found: {args.ref}", file=sys.stderr)
        return 1

    plu_by_name = load_plu_by_name(args.ref)
    print(f"PLU reference names: {len(plu_by_name)} ({args.ref})")
    if args.dry_run:
        print("DRY RUN — no writes")

    if args.db:
        if not args.db.is_file():
            print(f"error: DB not found: {args.db}", file=sys.stderr)
            return 1
        stats = backfill_db(
            args.db,
            plu_by_name,
            dry_run=args.dry_run,
            drop_plu_steps=args.drop_plu_steps,
        )
        print_stats(f"DB {args.db}", stats)

    if args.bundle:
        if not args.bundle.is_file():
            print(f"error: bundle not found: {args.bundle}", file=sys.stderr)
            return 1
        stats = backfill_bundle(
            args.bundle,
            plu_by_name,
            dry_run=args.dry_run,
            drop_plu_steps=args.drop_plu_steps,
        )
        print_stats(f"Bundle {args.bundle}", stats)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
