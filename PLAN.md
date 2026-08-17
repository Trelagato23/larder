# Larder — implementation plan (handoff)

Ordered backlog for continuing larder development. Written 2026-07-22 after a
full ricing + feature audit. Read `DESIGN.md` first — it is the visual contract
and this plan does not repeat it.

## Step 0 — commit the working tree (do this FIRST)

The repo has a large pile of **uncommitted work** predating this plan:
migrations 003–008, ingredient master, locations, production, auth, prep
sheets, this session's features (below), and both DESIGN.md/DEPLOY.md.
Everything below assumes a clean baseline. Commit (or split into logical
commits) before touching anything else.

Already done this session — **do not redo**:

- Web cooking-mode step timers — `server/src/static/index.html` (~lines 468-488
  CSS, 2240-2342 JS). Per-step ▶/⏸/↺ countdowns, independent, survive
  `renderRecipeDetail()` re-renders (state in `stepTimers` map), done pulse +
  WebAudio beep, `prefers-reduced-motion` opt-out on the pulse. Headless-tested
  only — eyeball it once in a browser via `larder serve`.
- TUI text wrapping — `tui/src/ui/recipe_detail.rs:357,435`. Recipe detail body
  + cooking mode now `.wrap(Wrap { trim: false })`. Scroll applies post-wrap;
  j/k still works (one keypress = one visual row).
- CLI `scale` — `tui/src/commands/cli.rs` (`Scale { id, servings, factor }`).
  `larder scale <recipe> <servings>` or `--factor <x>`, same id-or-name
  resolution as `show`/`cook`.
- Fraction parsing — `core/src/services/scaling.rs` `extract_leading_number`
  handles `1/2`, `1 1/2`, unicode `½ ¼ ¾ ⅓ ⅔ ⅛ ...` (incl. digit-adjacent
  `1½`). `scale_display_by_factor` rounds display to 3 dp. 19/19 core tests
  pass.

## Rules for every change

- Minimal diffs. Match existing file style (vanilla JS SPA, no deps; Rust:
  existing service patterns). No speculative abstraction.
- Never touch the real `larder.db` in the repo root. Test against a copy in a
  temp dir, or `--database sqlite:/tmp/...`.
- Verify: `cargo test -p larder-core`, `cargo build -p larder-tui`, and for web
  changes `node --check` on the extracted `<script>` block.
- Screen ricing must not leak into `@media print` / `printableDoc` — print
  stays white paper, black ink, Georgia (DESIGN.md).
- Any new animation needs a `prefers-reduced-motion` opt-out (DESIGN.md
  workflow step 6). Kitchen mode: zero decorative motion.
- Design changes: follow DESIGN.md's change workflow (tokens first, screenshot
  kitchen + manager at 768px, print-preview one prep sheet).

## Phase 1 — kitchen-visible wins

### 1.1 Allergen/dietary badges
- Add `allergens` to recipe model (`core/src/models/recipe.rs`) via new
  migration `009_allergens.sql` (TEXT, comma-separated or JSON array — match
  how tags are stored for consistency).
- Surface as badges on recipe detail (web + TUI) and — critically — on the
  **prep sheet print** (`server/src/routes/prep_sheet.rs` + the web print
  modal). Compliance-relevant: must be readable on paper.
- Manager-edit only (web recipe editor + TUI editor). Suggest a fixed
  vocabulary (gluten, dairy, egg, nuts, soy, sesame, shellfish) + free text.
- Acceptance: set allergens on a recipe, see badges in web detail, TUI detail,
  and printed prep sheet.

### 1.2 FTS5 search
- `core/src/services/recipe.rs:206` uses `LIKE %q%` across name/description/
  ingredient text; web fakes fuzzy matching client-side (`fuzzyScore` in
  index.html), so CLI/TUI/web return different results.
- Add FTS5 virtual table (recipes + ingredient text) in a migration, maintain
  via triggers or rebuild-on-write, ranked `bm25` results.
- Update CLI `search`, API search route, and web to use it so all three
  surfaces agree. Keep `LIKE` fallback if FTS table is missing.
- Acceptance: same query returns same ordering on CLI and web; typo-tolerant
  prefix matching works (`chkn` may still fail — that's fine, FTS is prefix
  not fuzzy; note it in README).

### 1.3 Culinary rounding of scaled quantities
- `scale_display_by_factor` emits `2.66 cups` style output. Add rounding to
  measurable steps (¼/⅓/½) **for volume/cup-like units only** — grams should
  stay numeric. Unit-aware: round to nearest ¼ for cups/tsp/tbsp, whole grams
  for g, 0.1 kg for kg.
- Lives in `core/src/services/scaling.rs`; unit tests for each unit class.
- Acceptance: ×1.33 of "2 cups" prints `2⅔ cups` (or `2.75`), grams print clean
  integers.

## Phase 2 — DESIGN.md runway (ricing)

Order matters: D4 (CSS extraction) makes D2/D5 much easier, but D1/D3 are
cheap. Do D1 → D3 → D4 → D2 → D5.

- **D1 remainder**: rename nav `Import` → `Data`; `More ▾` → `Plan`
  (production, meal plan, shopping); make tag quick-filter chips use the
  dept-stripe palette (`DEPT_STRIPE_COLORS` already exists ~line 1122).
- **D3**: remove the 6-theme picker (header + login + `THEMES` array +
  `applyTheme`/`renderThemeMenus` machinery). Office (manager) gets a
  Light/Dark toggle only; kitchen is locked light. Migrate: ignore old
  `larder_theme` localStorage values other than the two survivors.
- **D4**: extract the ~895-line inline `<style>` to
  `server/src/static/larder.css` (+ `tokens.css` for custom properties). Serve
  as static files. Pure refactor — no visual change. Verify with screenshots
  before/after.
- **D2**: `body.kitchen-mode` layout per DESIGN.md — minimal header (location
  + search + logout), full-bleed search, 720px single column, 48px touch
  targets, no costs/edit/theme chrome. Reference implementation:
  `server/src/static/prototypes/kitchen-board.html`. Default by role, with a
  pref to override.
- **D5**: typography swap (IBM Plex Sans body, Plex Mono for costs/qty,
  display serif per open decision below) + index-card surfaces (8px radius,
  top rule, dept left stripe baked in rather than a pref).
- While in there, fix the a11y gaps: `:focus-visible` styles for nav/cards/
  menu items, `tabindex`/`role="button"` on clickable cards, update
  `aria-expanded` in the nav-more/options toggles (currently hardcoded
  `"false"`), reduced-motion opt-outs for the pre-existing toast/hover
  animations.

## Phase 3 — TUI ricing

- **Theme module**: new `tui/src/ui/theme.rs` — one struct of semantic roles
  (header, accent, money, timer, difficulty, dept colors) using `Color::Rgb`
  with the co-op palette (brand `#b42318`, dept hexes from index.html ~1122).
  Replace the ~84 scattered `Color::` literals across the 8 UI files.
- **Polish on top**: brand-reverse list highlight (instead of `DarkGray`),
  remove the duplicate bordered "Recipes" header block in
  `recipe_list.rs:144-151`, `BorderType::Rounded`, dept-colored `#tag` spans
  (currently all Magenta, `recipe_detail.rs:277`), scrollbar + PgUp/PgDn in
  detail view, status-bar hints de-duplicated from per-view footers.
- **Open question**: long recipe descriptions clip in the fixed `Length(5)`
  header (`recipe_detail.rs:269`). Either grow header height conditionally or
  truncate with ellipsis deliberately — needs a human call.

## Phase 4 — data quality + small gaps

- **UOM density**: `core/src/services/uom.rs:120` has 8 hardcoded
  `contains("flour")` heuristics and `ml→g` assumes water (1:1) — silently
  wrong for oil/honey in pull lists. Add `g_per_cup` (nullable) to ingredient
  master; fall back to heuristics only when null. Manager-editable in web.
- **Cookbook remove**: no route to remove a recipe from a cookbook (write-only
  today) — add `DELETE` in `server/src/routes/cookbooks.rs` + web button.
- **CLI `tag remove`** — API has DELETE; `cli.rs` only has `add` (~line 76).
- **Prototypes mirror**: `prototypes/` only has `kitchen-board.html`; copy
  `office-board.html` + hub `index.html` from `server/src/static/prototypes/`
  or fix the claim in `DESIGN.md:124`.
- Delete empty `tui/src/app/` dir.

## Phase 5 — strategic (discuss before starting)

- **Computed nutrition**: kcal/macros per 100g on ingredient master → computed
  per-recipe nutrition. Replaces hand-entered `estimated_calories` (migration
  008). Natural fit now that UOM conversion exists.
- **On-hand inventory (L5)**: track stock per location; shopping-list
  generation subtracts on-hand. Locations already anchor per-store.
- **Import review step**: preview/dedupe before commit in the web import flow
  (`import.rs` is 684 lines, no dedupe on re-import).

## Open decisions (human picks, then update DESIGN.md)

1. Display serif: keep Fraunces vs Libre Baskerville (DESIGN.md open q1).
2. Kitchen default route: search-only home vs last-opened recipe.
3. Dept stripes: derive from primary tag vs manual recipe field.
4. TUI long-description header (Phase 3 above).

## Verification cheat sheet

```bash
cargo test -p larder-core        # unit tests (scaling, uom, + new)
cargo build -p larder-tui        # CLI + TUI compile
cargo build -p larder-server     # server compile
# web JS syntax:
sed -n '/<script>/,/<\/script>/p' server/src/static/index.html | sed '1d;$d' > /tmp/app.js && node --check /tmp/app.js
# smoke test (temp db, never the real one):
d=$(mktemp -d); cp larder.db "$d/"; (cd "$d" && larder list && larder scale "Some Recipe" 12); rm -rf "$d"
```
