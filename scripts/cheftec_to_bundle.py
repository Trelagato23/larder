#!/usr/bin/env python3
"""Convert ChefTec LexCo HTML export → larder full-bundle JSON.

Usage:
  python3 scripts/cheftec_to_bundle.py data/lexco-source/LexCo_Recipes.htm \\
      -o data/lexco-source/lexco-bundle.json

Optional:
  --limit N          only first N recipes (smoke test)
  --tag TAG          extra tag on every recipe (default: work)
  --no-master        skip top-level ingredient master list
"""

from __future__ import annotations

import argparse
import html as html_lib
import json
import re
import sys
from collections import OrderedDict
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

# HTML entities used for vulgar fractions in ChefTec dumps
FRACTION_ENTITIES = {
    "&#188;": "1/4",
    "&#189;": "1/2",
    "&#190;": "3/4",
    "&#188": "1/4",
    "&#189": "1/2",
    "&#190": "3/4",
    "¼": "1/4",
    "½": "1/2",
    "¾": "3/4",
    "⅓": "1/3",
    "⅔": "2/3",
    "⅛": "1/8",
    "⅜": "3/8",
    "⅝": "5/8",
    "⅞": "7/8",
}

DURATION_RE = re.compile(
    r"(?P<h>\d+)\s*(?:hours?|hrs?|h)\b|(?P<m>\d+)\s*(?:minutes?|mins?|m)\b",
    re.I,
)
TAG_RE = re.compile(r"<[^>]+>")
WS_RE = re.compile(r"\s+")


def decode_entities(s: str) -> str:
    for ent, repl in FRACTION_ENTITIES.items():
        s = s.replace(ent, f" {repl} ")
    s = html_lib.unescape(s)
    return WS_RE.sub(" ", s).strip()


def tag_text(block: str, tag: str) -> str | None:
    m = re.search(rf"<{tag}(?:\s[^>]*)?>(.*?)</{tag}>", block, re.I | re.S)
    if not m:
        return None
    return decode_entities(m.group(1))


def tag_attr(block: str, tag: str) -> str | None:
    """Self-closing-ish ChefTec attrs: <CTPREPARED=01/01/2009> or <CTITEMNAME=oats>…"""
    m = re.search(rf"<{tag}=([^>]*)>", block, re.I)
    if not m:
        return None
    return decode_entities(m.group(1).strip().strip('"'))


def parse_duration_minutes(text: str | None) -> int | None:
    if not text:
        return None
    t = text.strip().lower()
    if not t or t in {"n/a", "na", "-", "none", "0", "0 minutes"}:
        return None
    total = 0
    found = False
    for m in DURATION_RE.finditer(t):
        found = True
        if m.group("h"):
            total += int(m.group("h")) * 60
        if m.group("m"):
            total += int(m.group("m"))
    if found:
        return total or None
    # bare number → minutes
    m = re.fullmatch(r"(\d+)", t)
    if m:
        return int(m.group(1)) or None
    return None


def parse_quantity(raw: str | None) -> Decimal | None:
    if raw is None:
        return None
    s = decode_entities(raw)
    if not s:
        return None
    # "1 1/2", "2 3/4", "1/4", "3.5"
    s = s.replace(" ", " ").strip()
    parts = s.split()
    try:
        total = Decimal(0)
        for p in parts:
            if "/" in p:
                a, b = p.split("/", 1)
                total += Decimal(a) / Decimal(b)
            else:
                total += Decimal(p)
        return total
    except (InvalidOperation, ZeroDivisionError, ValueError):
        return None


def clean_unit(unit: str | None) -> str | None:
    if not unit:
        return None
    u = decode_entities(unit).strip().rstrip(".")
    return u or None


def slug_tag(name: str) -> str:
    s = name.lower().strip()
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-")[:48] or "misc"


# ChefTec day-board noise — never keep as tags
_DROP_TAG_RE = re.compile(
    r"^(hb-menus|hb[1-7]-?(sun|mon|tues|wednes|thurs|fri|satur)?day|demo(-hot-bar)?)$",
    re.I,
)


def normalize_import_tag(raw: str) -> str | None:
    """Map category strings to kitchen tags; drop hb day-menu noise."""
    t = slug_tag(raw)
    if not t or t == "work":
        return None
    if _DROP_TAG_RE.match(t) or (t.startswith("hb") and any(
        d in t for d in (
            "sunday", "monday", "tuesday", "wednesday",
            "thursday", "friday", "saturday", "menus",
        )
    )):
        return "hot-bar"
    # common aliases — Grab & Go (cold) vs Hot Bar (hot) are exclusive
    aliases = {
        "hb-menus": "hot-bar",
        "grab-go": "grab-go",
        "grab-and-go": "grab-go",
        "grab-go-hot-bar": "hot-bar",  # hybrid ChefTec tag → hot only
    }
    return aliases.get(t, t)


def strip_html(s: str) -> str:
    s = TAG_RE.sub(" ", s)
    return decode_entities(s)


def parse_steps(body: str) -> list[dict[str, Any]]:
    # Prefer visible Procedure block before RTF comment
    m = re.search(
        r"Procedure</FONT>\s*<FONT[^>]*>\s*(.*?)\s*</FONT>\s*<!--",
        body,
        re.I | re.S,
    )
    if not m:
        m = re.search(
            r"Procedure</FONT>\s*<FONT[^>]*>\s*(.*?)\s*(?:<CTAUTHOR|Prepared by)",
            body,
            re.I | re.S,
        )
    if not m:
        # RTF fallback: \par lines inside CTProcedureRTF
        rtf = re.search(r"<CTProcedureRTF>(.*?)</CTProcedureRTF>", body, re.I | re.S)
        if not rtf:
            return []
        raw = rtf.group(1)
        # crude RTF: take text after \par
        parts = re.split(r"\\par\b", raw)
        lines = []
        for p in parts:
            p = re.sub(r"\\[a-z]+\d*\s?", " ", p, flags=re.I)
            p = re.sub(r"[{}]", " ", p)
            p = strip_html(p)
            if p:
                lines.append(p)
        steps = []
        for i, line in enumerate(lines, 1):
            if line.lower() in {"assembly:", "procedure:", "method:"}:
                continue
            steps.append({"position": i, "instruction": line})
        return steps

    chunk = m.group(1)
    # Split on <p> boundaries ChefTec uses
    bits = re.split(r"<p[^>]*>", chunk, flags=re.I)
    steps: list[dict[str, Any]] = []
    for bit in bits:
        line = strip_html(bit)
        if not line:
            continue
        low = line.lower().rstrip(":")
        if low in {"assembly", "procedure", "method", "directions"}:
            continue
        steps.append({"position": len(steps) + 1, "instruction": line})
    return steps


def parse_ingredients(body: str) -> list[dict[str, Any]]:
    items = []
    for m in re.finditer(r"<CTRECPITEM=\d+>(.*?)</CTRECPITEM>", body, re.I | re.S):
        block = m.group(1)
        qty_raw = tag_text(block, "CTQTY")
        unit = clean_unit(tag_text(block, "CTUNIT"))
        # display name may differ from CTITEMNAME attr
        name_m = re.search(
            r"<CTITEMNAME(?:=([^>]*))?>(.*?)</CTITEMNAME>", block, re.I | re.S
        )
        if name_m:
            attr_name = decode_entities(name_m.group(1)) if name_m.group(1) else None
            display_name = decode_entities(name_m.group(2)) if name_m.group(2) else None
            ingredient = (display_name or attr_name or "").strip()
        else:
            ingredient = ""
        if not ingredient:
            continue
        pre = tag_text(block, "CTPREINSTR") or ""
        post = tag_text(block, "CTPOSTINSTR") or ""
        note_parts = [p for p in (pre, post) if p]
        note = "; ".join(note_parts) if note_parts else None

        qty = parse_quantity(qty_raw)
        # Build display line
        disp_parts = []
        if qty is not None:
            # pretty decimal: drop trailing zeros
            q = format(qty.normalize(), "f").rstrip("0").rstrip(".") if qty == qty.to_integral() else format(qty, "f").rstrip("0").rstrip(".")
            # better: use normalize carefully
            q = f"{qty:f}".rstrip("0").rstrip(".") if "." in f"{qty:f}" else f"{qty:f}"
            disp_parts.append(q)
        if unit:
            disp_parts.append(unit)
        disp_parts.append(ingredient)
        if note:
            disp_parts.append(f"({note})")
        display = " ".join(disp_parts)

        items.append(
            {
                "ingredient": ingredient,
                "quantity": str(qty) if qty is not None else None,
                "unit": unit,
                "note": note,
                "display": display,
                "master_ingredient_name": ingredient,
            }
        )
    return items


def parse_recipe(body: str) -> dict[str, Any] | None:
    name = tag_text(body, "CTRECPNAME")
    if not name:
        return None

    categories = tag_text(body, "CTLIST1") or ""
    locations = tag_text(body, "CTLIST2") or ""
    tools = tag_text(body, "CTLIST4") or ""
    plu = tag_text(body, "CTPLU")
    author = tag_text(body, "CTAUTHOR")
    yield_qty = tag_text(body, "CTYIELDQTY")
    yield_unit = clean_unit(tag_text(body, "CTYIELDUNIT"))
    portion_qty = tag_text(body, "CTPORTIONQTY")
    portion_unit = clean_unit(tag_text(body, "CTPORTIONUNIT"))
    num_portions = tag_text(body, "CTNUMPORTIONS")
    prep = tag_text(body, "CTBOX1")
    cook = tag_text(body, "CTBOX2")
    finish = tag_text(body, "CTBOX3")
    shelf = tag_text(body, "CTBOX4")

    prep_m = parse_duration_minutes(prep)
    cook_m = parse_duration_minutes(cook)
    finish_m = parse_duration_minutes(finish)
    total = None
    if any(x is not None for x in (prep_m, cook_m, finish_m)):
        total = (prep_m or 0) + (cook_m or 0) + (finish_m or 0) or None

    # servings: prefer Num Portions
    servings = 1
    if num_portions:
        try:
            servings = max(1, int(float(num_portions.replace(",", ""))))
        except ValueError:
            servings = 1
    elif portion_qty:
        try:
            servings = max(1, int(float(portion_qty.replace(",", ""))))
        except ValueError:
            pass

    yq = parse_quantity(yield_qty)

    desc_bits = []
    if plu:
        desc_bits.append(f"PLU {plu}")
    if portion_qty or portion_unit:
        desc_bits.append(
            f"portion {portion_qty or ''} {portion_unit or ''}".strip()
        )
    if shelf:
        desc_bits.append(f"shelf {shelf}")
    if tools:
        desc_bits.append(f"tools: {tools}")
    if locations:
        desc_bits.append(f"station: {locations}")
    description = " · ".join(desc_bits) if desc_bits else None

    tags: list[str] = []
    seen_tags: set[str] = set()
    for cat in re.split(r"[,;/|]+", categories):
        cat = cat.strip()
        if not cat:
            continue
        t = normalize_import_tag(cat)
        if t and t not in seen_tags:
            seen_tags.add(t)
            tags.append(t)
    if locations:
        t = normalize_import_tag(locations)
        if t and t not in seen_tags:
            seen_tags.add(t)
            tags.append(t)

    ingredients = parse_ingredients(body)
    steps = parse_steps(body)

    recipe: dict[str, Any] = {
        "name": name,
        "description": description,
        "servings": servings,
        "prep_time_minutes": prep_m,
        "cook_time_minutes": cook_m,
        "total_time_minutes": total,
        "author": author,
        "yield_quantity": str(yq) if yq is not None else None,
        "yield_unit": yield_unit,
        "tags": tags,
        "ingredients": ingredients,
        "steps": steps,
        "source_url": None,
    }
    # drop null-ish optionals for cleaner JSON (import tolerates either way)
    return {k: v for k, v in recipe.items() if v is not None and v != []}


def parse_file(path: Path, limit: int | None = None) -> list[dict[str, Any]]:
    raw = path.read_bytes()
    # ChefTec dumps are Windows-1252 / latin-1 with occasional binary nulls
    text = raw.replace(b"\x00", b"").decode("latin-1", errors="replace")

    recipes: list[dict[str, Any]] = []
    # Each recipe body is between <CTRecipe>…</CTRecipe> after the TOC
    for m in re.finditer(r"<CTRecipe>(.*?)</CTRecipe>", text, re.I | re.S):
        body = m.group(1)
        if "<CTRECPNAME>" not in body and "<CTRECPNAME" not in body.upper():
            # Some open tags sit before name on next block; still try
            pass
        rec = parse_recipe(body)
        if not rec:
            continue
        recipes.append(rec)
        if limit is not None and len(recipes) >= limit:
            break
    return recipes


def build_bundle(
    recipes: list[dict[str, Any]],
    *,
    extra_tags: list[str],
    include_master: bool,
) -> dict[str, Any]:
    masters: OrderedDict[str, dict[str, Any]] = OrderedDict()

    for r in recipes:
        # ensure tags
        tags = list(r.get("tags") or [])
        for t in extra_tags:
            if t not in tags:
                tags.insert(0, t)
        r["tags"] = tags

        for ing in r.get("ingredients") or []:
            name = ing.get("ingredient") or ""
            key = name.casefold().strip()
            if not key:
                continue
            if key not in masters:
                masters[key] = {
                    "name": name,
                    "default_unit": ing.get("unit"),
                }
            elif not masters[key].get("default_unit") and ing.get("unit"):
                masters[key]["default_unit"] = ing["unit"]

            # serde Decimal fields as strings is fine for rust_decimal
            if ing.get("quantity") is not None:
                ing["quantity"] = ing["quantity"]  # already str
            # remove null keys from ingredient lines
            for nk in list(ing.keys()):
                if ing[nk] is None:
                    del ing[nk]

        # clean recipe nulls already done; ensure yield as string ok
        for nk in ("prep_time_minutes", "cook_time_minutes", "total_time_minutes"):
            if r.get(nk) is None and nk in r:
                del r[nk]

    bundle: dict[str, Any] = {
        "larder_version": 1,
        "exported_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "ingredients": list(masters.values()) if include_master else [],
        "location_prices": [],
        "recipes": recipes,
    }
    return bundle


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("html", type=Path, help="ChefTec LexCo_Recipes.htm (or similar)")
    ap.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("lexco-bundle.json"),
        help="Output bundle path",
    )
    ap.add_argument("--limit", type=int, default=None, help="Only first N recipes")
    ap.add_argument(
        "--tag",
        action="append",
        default=[],
        help="Tag every recipe (repeatable). Default: work",
    )
    ap.add_argument(
        "--no-master",
        action="store_true",
        help="Do not emit top-level ingredient master entries",
    )
    args = ap.parse_args()

    if not args.html.is_file():
        print(f"error: not a file: {args.html}", file=sys.stderr)
        return 1

    tags = args.tag or ["work"]
    print(f"Parsing {args.html} …", file=sys.stderr)
    recipes = parse_file(args.html, limit=args.limit)
    print(f"  recipes: {len(recipes)}", file=sys.stderr)

    with_ings = sum(1 for r in recipes if r.get("ingredients"))
    with_steps = sum(1 for r in recipes if r.get("steps"))
    print(f"  with ingredients: {with_ings}", file=sys.stderr)
    print(f"  with steps: {with_steps}", file=sys.stderr)

    bundle = build_bundle(
        recipes, extra_tags=tags, include_master=not args.no_master
    )
    print(
        f"  master ingredients: {len(bundle['ingredients'])}",
        file=sys.stderr,
    )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(bundle, indent=2, ensure_ascii=False) + "\n")
    print(f"Wrote {args.output} ({args.output.stat().st_size // 1024} KB)", file=sys.stderr)

    # quick sample
    if recipes:
        s = recipes[0]
        print(
            f"  sample: {s['name']!r} "
            f"servings={s.get('servings')} "
            f"ings={len(s.get('ingredients') or [])} "
            f"steps={len(s.get('steps') or [])} "
            f"tags={s.get('tags')}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
