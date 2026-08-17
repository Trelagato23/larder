#!/usr/bin/env python3
"""
Fix or hide incomplete recipes in a Larder SQLite DB.

Rules:
  - Empty stubs (no ingredients, no real steps) → DELETE
  - Simple mix/assemble recipes missing method → add a minimal step
  - Roasted Squash Seeds (has method, no ings) → add ingredients from method
  - Complex dishes with no usable method → tag #incomplete (UI hides them)
  - Junk-only steps (Unknown, "1. Place") → remove; reclassify

Usage:
  python3 scripts/fix_incomplete_recipes.py ~/.local/share/larder/work.db
  python3 scripts/fix_incomplete_recipes.py --dry-run ~/.local/share/larder/work.db
"""
from __future__ import annotations

import argparse
import re
import sqlite3
import sys
import uuid
from pathlib import Path

JUNK_STEP_RE = re.compile(
    r"^(unknown|instructions?:?|n/?a|none|tbd|1\.\s*place\s*)$",
    re.I,
)
JUNK_SUBSTR = (
    "no method in source",
    "no method",
)


def is_junk_step(text: str) -> bool:
    """True only for empty / placeholder steps safe to delete."""
    t = (text or "").strip()
    if not t:
        return True
    if JUNK_STEP_RE.match(t):
        return True
    low = t.lower()
    if any(s in low for s in JUNK_SUBSTR):
        return True
    # Lone labels with no instruction body
    if low in ("instructions", "instructions:", "method", "method:", "procedure"):
        return True
    if low.startswith("pack out info") and len(t) < 24:
        return True
    return False


def is_real_step(text: str) -> bool:
    """Usable cook instruction (not a junk placeholder). PLU / pack-out count as real."""
    t = (text or "").strip()
    if is_junk_step(t):
        return False
    # Very short noise labels only
    if len(t) < 3:
        return False
    return True


def is_simple_fixable(name: str, ings: int) -> str | None:
    """Return a method template key if we can auto-write a safe step."""
    n = name.lower()
    if ings <= 0:
        return None
    if any(k in n for k in ("rub", "spice blend", "spice mix", "seasoning", "dry mix")):
        return "rub"
    if "tray" in n and ings <= 8:
        return "tray"
    if any(k in n for k in ("glaze", "simple syrup")) and ings <= 5:
        return "glaze"
    if re.search(r"\b(oil|butter)\b", n) and ings <= 4:
        return "infuse"
    if ings <= 4 and any(k in n for k in ("dip", "spread", "dressing", "vinaigrette")):
        return "mix"
    return None


METHOD = {
    "rub": (
        "Combine all ingredients thoroughly until evenly mixed. "
        "Store in an airtight container."
    ),
    "tray": (
        "Prep each item as noted (dice, slice). "
        "Arrange on tray for service / grab-and-go. Cover and refrigerate until needed."
    ),
    "glaze": (
        "Combine ingredients in a saucepan. Bring to a simmer and cook until slightly "
        "thickened. Cool and refrigerate until needed."
    ),
    "infuse": (
        "Combine ingredients in a pan over medium-low heat until fragrant / colored. "
        "Strain if needed. Cool and store."
    ),
    "mix": (
        "Combine all ingredients and mix until smooth and evenly blended. "
        "Taste, adjust seasoning, and refrigerate until needed."
    ),
}


def ensure_tag(conn: sqlite3.Connection, name: str, color: str = "#6b635a") -> str:
    row = conn.execute("SELECT id FROM tags WHERE name = ?", (name,)).fetchone()
    if row:
        return row[0]
    tid = str(uuid.uuid4())
    conn.execute("INSERT INTO tags (id, name, color) VALUES (?, ?, ?)", (tid, name, color))
    return tid


def tag_recipe(conn: sqlite3.Connection, recipe_id: str, tag_id: str) -> None:
    exists = conn.execute(
        "SELECT 1 FROM recipe_tags WHERE recipe_id = ? AND tag_id = ?",
        (recipe_id, tag_id),
    ).fetchone()
    if not exists:
        conn.execute(
            "INSERT INTO recipe_tags (recipe_id, tag_id) VALUES (?, ?)",
            (recipe_id, tag_id),
        )


def clear_steps(conn: sqlite3.Connection, recipe_id: str) -> None:
    conn.execute("DELETE FROM recipe_steps WHERE recipe_id = ?", (recipe_id,))


def add_step(conn: sqlite3.Connection, recipe_id: str, instruction: str, position: int = 1) -> None:
    conn.execute(
        "INSERT INTO recipe_steps (id, recipe_id, position, instruction, timer_seconds) VALUES (?, ?, ?, ?, NULL)",
        (str(uuid.uuid4()), recipe_id, position, instruction),
    )


def add_ingredient(
    conn: sqlite3.Connection,
    recipe_id: str,
    ingredient: str,
    display: str,
    quantity: str | None = None,
    unit: str | None = None,
    note: str | None = None,
) -> None:
    conn.execute(
        """
        INSERT INTO recipe_ingredients
          (id, recipe_id, ingredient, quantity, unit, note, display, category)
        VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
        """,
        (str(uuid.uuid4()), recipe_id, ingredient, quantity, unit, note, display),
    )


def recipe_stats(conn: sqlite3.Connection, recipe_id: str) -> tuple[int, int, list[str]]:
    ings = conn.execute(
        "SELECT COUNT(*) FROM recipe_ingredients WHERE recipe_id = ?", (recipe_id,)
    ).fetchone()[0]
    steps = conn.execute(
        "SELECT instruction FROM recipe_steps WHERE recipe_id = ? ORDER BY position, rowid",
        (recipe_id,),
    ).fetchall()
    texts = [s[0] or "" for s in steps]
    real = sum(1 for t in texts if is_real_step(t))
    return ings, real, texts


def fix_roasted_squash_seeds(conn: sqlite3.Connection, recipe_id: str) -> None:
    # Has method text mentioning seeds + kosher salt; no ingredient lines in export.
    n = conn.execute(
        "SELECT COUNT(*) FROM recipe_ingredients WHERE recipe_id = ?", (recipe_id,)
    ).fetchone()[0]
    if n:
        return
    add_ingredient(
        conn,
        recipe_id,
        "squash seeds",
        "squash seeds (from roasted squash, rinsed)",
        note="from roasted squash, rinsed",
    )
    add_ingredient(
        conn,
        recipe_id,
        "kosher salt",
        "kosher salt (to taste)",
        note="to taste",
    )


def process(db_path: Path, dry_run: bool) -> int:
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    incomplete_tag = ensure_tag(conn, "incomplete", "#8a8178")

    recipes = conn.execute("SELECT id, name FROM recipes ORDER BY name").fetchall()
    counts = {"delete": 0, "fix": 0, "hide": 0, "clean_junk": 0, "ok": 0}

    for r in recipes:
        rid, name = r["id"], r["name"]
        ings, real, texts = recipe_stats(conn, rid)

        # Strip placeholder junk steps only (Unknown, empty, "no method"…)
        junk_ids = []
        for row in conn.execute(
            "SELECT id, instruction FROM recipe_steps WHERE recipe_id = ?", (rid,)
        ):
            if is_junk_step(row[1] or ""):
                junk_ids.append(row[0])
        if junk_ids:
            if not dry_run:
                for jid in junk_ids:
                    conn.execute("DELETE FROM recipe_steps WHERE id = ?", (jid,))
            counts["clean_junk"] += 1
            ings, real, texts = recipe_stats(conn, rid)

        # Empty stubs
        if ings == 0 and real == 0:
            print(f"  DELETE  {name}")
            if not dry_run:
                conn.execute("DELETE FROM recipes WHERE id = ?", (rid,))
            counts["delete"] += 1
            continue

        # Method-only (e.g. Roasted Squash Seeds)
        if name == "Roasted Squash Seeds" and ings == 0 and real > 0:
            print(f"  FIX     {name}  (add ingredients from method)")
            if not dry_run:
                fix_roasted_squash_seeds(conn, rid)
            counts["fix"] += 1
            continue

        if ings > 0 and real == 0:
            kind = is_simple_fixable(name, ings)
            if kind:
                print(f"  FIX     {name}  ({kind} method)")
                if not dry_run:
                    clear_steps(conn, rid)
                    add_step(conn, rid, METHOD[kind], 1)
                    # untag incomplete if previously set
                    conn.execute(
                        "DELETE FROM recipe_tags WHERE recipe_id = ? AND tag_id = ?",
                        (rid, incomplete_tag),
                    )
                counts["fix"] += 1
            else:
                print(f"  HIDE    {name}  (ings={ings}, no method)")
                if not dry_run:
                    clear_steps(conn, rid)  # drop Unknown / stubs
                    tag_recipe(conn, rid, incomplete_tag)
                counts["hide"] += 1
            continue

        if ings == 0 and real > 0:
            # Unusual: method without ings — hide unless we special-cased above
            print(f"  HIDE    {name}  (method only, no ingredients)")
            if not dry_run:
                tag_recipe(conn, rid, incomplete_tag)
            counts["hide"] += 1
            continue

        counts["ok"] += 1

    if dry_run:
        conn.rollback()
        print("\n(dry-run — no changes written)")
    else:
        conn.commit()
        # Best-effort FTS rebuild if virtual table exists
        try:
            has_fts = conn.execute(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='recipe_fts'"
            ).fetchone()
            if has_fts:
                # leave FTS to app; deletes cascade may leave orphans depending on triggers
                pass
        except sqlite3.Error:
            pass

    conn.close()
    print(
        f"\nDone: delete={counts['delete']} fix={counts['fix']} "
        f"hide={counts['hide']} junk_cleaned≈{counts['clean_junk']} ok={counts['ok']}"
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("db", type=Path, help="Path to larder SQLite DB (e.g. work.db)")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if not args.db.exists():
        print(f"DB not found: {args.db}", file=sys.stderr)
        return 1
    print(f"{'DRY-RUN ' if args.dry_run else ''}Scanning {args.db}")
    return process(args.db, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
