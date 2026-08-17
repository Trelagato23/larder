#!/usr/bin/env python3
"""
Backfill hot-bar Service steps (post-reheat) into recipe_steps.

Every #hot-bar recipe gets a final step the SPA can parse as Service:
  - Keep existing "To serve…" / "At service…" last steps
  - Rewrite clear serve-time last steps to "To serve, reheat and …"
  - Otherwise append: "To serve: reheat and serve as is."

Does not invent garnishes. Does not move mid-cook garnish language.

Usage:
  python3 scripts/backfill_hotbar_service_steps.py --dry-run ~/.local/share/larder/work.db
  python3 scripts/backfill_hotbar_service_steps.py ~/.local/share/larder/work.db
"""
from __future__ import annotations

import argparse
import re
import sqlite3
import sys
import uuid
from pathlib import Path

DEFAULT_SERVICE = "To serve: reheat and serve as is."

SERVICE_HEADER_RE = re.compile(
    r"^(To\s+serve|At\s+service|For\s+service|Just\s+before\s+service)\b",
    re.I,
)

# Last-step patterns that are already serve-time finishes (rewrite, don't append).
REWRITEABLE_LAST_RE = re.compile(
    r"^(?:"
    r"Just\s+before\s+serv(?:e|ing)"
    r"|When\s+serv(?:e|ing)"
    r"|After\s+heat(?:ing)?"
    r"|At\s+service"
    r"|For\s+service"
    r"|To\s+serve"
    r"|Garnish(?:\s+with)?"
    r")\b",
    re.I,
)

# Strip leading serve-time phrase to keep the finish action.
LEADING_SERVE_PHRASE_RE = re.compile(
    r"^(?:"
    r"Just\s+before\s+serv(?:e|ing)"
    r"|When\s+serv(?:e|ing)"
    r"|After\s+heat(?:ing)?"
    r"|At\s+service"
    r"|For\s+service"
    r"|To\s+serve"
    r"|Garnish(?:\s+with)?"
    r")\b[,:]?\s*",
    re.I,
)

JUNK_STEP_RE = re.compile(
    r"^(unknown|instructions?:?|n/?a|none|tbd|1\.\s*place\s*)$",
    re.I,
)


def is_junk_step(text: str) -> bool:
    t = (text or "").strip()
    if not t:
        return True
    if JUNK_STEP_RE.match(t):
        return True
    low = t.lower()
    if "no method" in low:
        return True
    if low in ("instructions", "instructions:", "method", "method:", "procedure"):
        return True
    if low.startswith("pack out info") and len(t) < 24:
        return True
    return False


def is_service_header(text: str) -> bool:
    return bool(SERVICE_HEADER_RE.match((text or "").strip()))


def normalize_service_step(text: str) -> str:
    """Turn a serve-time last step into a To serve, reheat … Service header step."""
    t = (text or "").strip()
    if is_service_header(t):
        # Ensure reheat is mentioned when it's a finish line
        if re.search(r"\breheat\b", t, re.I):
            return t
        # "To serve, drizzle…" → "To serve, reheat and drizzle…"
        rest = LEADING_SERVE_PHRASE_RE.sub("", t).strip()
        rest = rest.lstrip(",: ").strip()
        if not rest:
            return DEFAULT_SERVICE
        if rest.lower().startswith("reheat"):
            return f"To serve, {rest[0].lower() + rest[1:]}" if rest[0].isupper() else f"To serve, {rest}"
        return f"To serve, reheat and {rest[0].lower() + rest[1:] if rest[0].isupper() else rest}"

    rest = LEADING_SERVE_PHRASE_RE.sub("", t).strip()
    rest = rest.lstrip(",: ").strip()
    # "Garnish with X" → leading strip may leave "X" if pattern was Garnish with
    if re.match(r"^Garnish(?:\s+with)?\b", t, re.I) and rest and not rest.lower().startswith("with"):
        # LEADING already ate "Garnish with"; rest is the garnish target
        rest = f"garnish with {rest}"
    elif re.match(r"^Garnish\b", t, re.I) and not rest.lower().startswith("garnish"):
        rest = f"garnish {rest}" if rest else "garnish"

    if not rest:
        return DEFAULT_SERVICE
    if rest.lower().startswith("reheat"):
        body = rest[0].lower() + rest[1:] if rest[0].isupper() else rest
        return f"To serve, {body}"
    body = rest[0].lower() + rest[1:] if rest[0].isupper() else rest
    # Drop trailing "for service" / "to serve" noise
    body = re.sub(r"\s*,?\s*(for\s+service|to\s+serve)\.?\s*$", "", body, flags=re.I).strip()
    body = body.rstrip(".")
    return f"To serve, reheat and {body}."


def is_rewriteable_last(text: str) -> bool:
    t = (text or "").strip()
    if not t or is_junk_step(t):
        return False
    if is_service_header(t):
        return False  # already ok (caller handles)
    # Only rewrite when the *whole* last step is a serve-time instruction,
    # not a long cook step that happens to mention garnish.
    if REWRITEABLE_LAST_RE.match(t) and len(t) <= 280:
        return True
    if len(t) <= 160 and re.search(
        r"\b(just\s+before\s+serv(?:e|ing)|when\s+serv(?:e|ing)|after\s+heat(?:ing)?|"
        r"at\s+service|for\s+service)\b",
        t,
        re.I,
    ):
        return True
    return False


def hot_bar_recipes(conn: sqlite3.Connection) -> list[sqlite3.Row]:
    return conn.execute(
        """
        SELECT r.id, r.name
        FROM recipes r
        JOIN recipe_tags rt ON rt.recipe_id = r.id
        JOIN tags t ON t.id = rt.tag_id AND lower(t.name) = 'hot-bar'
        WHERE NOT EXISTS (
            SELECT 1 FROM recipe_tags rt2
            JOIN tags t2 ON t2.id = rt2.tag_id AND lower(t2.name) = 'incomplete'
            WHERE rt2.recipe_id = r.id
        )
        ORDER BY r.name
        """
    ).fetchall()


def get_steps(conn: sqlite3.Connection, recipe_id: str) -> list[sqlite3.Row]:
    return conn.execute(
        """
        SELECT id, position, instruction
        FROM recipe_steps
        WHERE recipe_id = ?
        ORDER BY position, rowid
        """,
        (recipe_id,),
    ).fetchall()


def next_position(steps: list[sqlite3.Row]) -> int:
    if not steps:
        return 1
    return max(int(s["position"] or 0) for s in steps) + 1


def dedupe_service_steps(conn: sqlite3.Connection, recipe_id: str, steps: list[sqlite3.Row]) -> int:
    """Remove empty/duplicate Service-header steps; keep the last good one."""
    service_idxs = [
        i
        for i, s in enumerate(steps)
        if is_service_header(s["instruction"] or "") or is_rewriteable_last(s["instruction"] or "")
    ]
    if len(service_idxs) <= 1:
        return 0
    # Keep last; delete earlier service-ish steps that are short duplicates
    keep = service_idxs[-1]
    deleted = 0
    for i in service_idxs[:-1]:
        text = (steps[i]["instruction"] or "").strip()
        if is_service_header(text) or (is_rewriteable_last(text) and len(text) < 200):
            # Only drop if clearly a service line, not mixed cook
            if len(text) <= 280:
                conn.execute("DELETE FROM recipe_steps WHERE id = ?", (steps[i]["id"],))
                deleted += 1
    return deleted


def process(db_path: Path, dry_run: bool) -> int:
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row

    counts = {
        "ok": 0,
        "append": 0,
        "rewrite": 0,
        "normalize": 0,
        "dedupe": 0,
        "no_steps": 0,
    }

    recipes = hot_bar_recipes(conn)
    print(f"{'DRY-RUN ' if dry_run else ''}Scanning {db_path} — {len(recipes)} hot-bar recipes")

    for r in recipes:
        rid, name = r["id"], r["name"]
        steps = get_steps(conn, rid)
        real = [s for s in steps if not is_junk_step(s["instruction"] or "")]

        if not real:
            print(f"  SKIP    {name}  (no real steps)")
            counts["no_steps"] += 1
            continue

        # Dedupe first (on current rows)
        if not dry_run:
            n = dedupe_service_steps(conn, rid, steps)
            if n:
                counts["dedupe"] += n
                steps = get_steps(conn, rid)
                real = [s for s in steps if not is_junk_step(s["instruction"] or "")]

        last = real[-1]
        last_text = (last["instruction"] or "").strip()

        if is_service_header(last_text):
            # Optionally inject "reheat" if missing and it's a finish (not serve-as-is)
            if re.search(r"\breheat\b", last_text, re.I) or re.search(
                r"serve as is", last_text, re.I
            ):
                print(f"  OK      {name}")
                counts["ok"] += 1
                continue
            new_text = normalize_service_step(last_text)
            if new_text == last_text:
                print(f"  OK      {name}")
                counts["ok"] += 1
                continue
            print(f"  NORM    {name}")
            print(f"          → {new_text}")
            if not dry_run:
                conn.execute(
                    "UPDATE recipe_steps SET instruction = ? WHERE id = ?",
                    (new_text, last["id"]),
                )
            counts["normalize"] += 1
            continue

        if is_rewriteable_last(last_text):
            new_text = normalize_service_step(last_text)
            print(f"  REWRITE {name}")
            print(f"          was: {last_text[:120]}")
            print(f"          → {new_text}")
            if not dry_run:
                conn.execute(
                    "UPDATE recipe_steps SET instruction = ? WHERE id = ?",
                    (new_text, last["id"]),
                )
            counts["rewrite"] += 1
            continue

        # Append default Service step
        print(f"  APPEND  {name}")
        if not dry_run:
            conn.execute(
                """
                INSERT INTO recipe_steps (id, recipe_id, position, instruction, timer_seconds)
                VALUES (?, ?, ?, ?, NULL)
                """,
                (str(uuid.uuid4()), rid, next_position(steps), DEFAULT_SERVICE),
            )
        counts["append"] += 1

    if dry_run:
        conn.rollback()
        print("\n(dry-run — no changes written)")
    else:
        conn.commit()

    conn.close()
    print(
        f"\nDone: ok={counts['ok']} append={counts['append']} "
        f"rewrite={counts['rewrite']} normalize={counts['normalize']} "
        f"dedupe={counts['dedupe']} no_steps={counts['no_steps']}"
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("db", type=Path, help="Path to Larder SQLite DB")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if not args.db.exists():
        print(f"DB not found: {args.db}", file=sys.stderr)
        return 1
    return process(args.db, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
