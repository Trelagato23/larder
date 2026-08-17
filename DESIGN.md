# Larder — design contract

Source of truth for visual identity, layout modes, and ricing workflow.
Homelab dashboard rules live in `projects/homelab/config/dashboard-vision-contract.md`; this file is **only for Larder**.

## Product + audience

| | |
|---|---|
| **What** | Shared store recipe book — lookup, scale, prep, cost (managers) |
| **Where** | Co-op kitchen tablets (Elmwood, Hertel), manager back-office |
| **Primary user** | Kitchen staff with wet/gloved hands, 3 seconds to find a recipe |
| **Secondary user** | Manager editing costs, imports, production pull lists |

The page's single job on the floor: **find the right recipe and print a scaled prep sheet**.

## Honest audit (current ricing)

**Working**
- CSS custom properties — themes are structurally sound
- 44px touch targets, kitchen gets larger recipe cards
- Role gating (manager chrome hidden from kitchen)
- Print preview modal (in-app, no pop-up roulette)
- Co-op red default reads as "store" not "personal recipe blog"

**Not working**
- **Six theme picker** — personal-homelab energy on a shared store tool; kitchen shouldn't pick Midnight vs Ember
- **Fraunces + cream + red** — correct co-op vibe but generic "2024 warm serif app" (see frontend-design skill defaults)
- **Header density** — location + user + role pill + theme + 4 nav items on one sticky bar
- **Import/export hidden** under nav label "Import" (data ops ≠ import only)
- **~600 lines CSS inline** in `index.html` — iteration is painful; no component boundaries
- **Kitchen vs manager** — same layout, slightly bigger text; not a mode switch
- **No dept visual language** — `#bakery` tag exists but cards don't look different from `#dinner`

## Direction: two modes, one brand

Drop the six-theme picker for production. Ship **two locked modes**:

```
┌─────────────────────────────────────────────────────────┐
│  KITCHEN MODE (default for kitchen@)                    │
│  • Full-bleed search, minimal header                    │
│  • Recipe tiles: name + time + dept stripe only         │
│  • Light only — high contrast for fluorescent kitchens  │
│  • No costs, no edit, no theme picker                 │
│  • Primary actions: Scale → Prep sheet → Cook          │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  OFFICE MODE (default for manager@)                     │
│  • Compact header, full nav                             │
│  • Costs visible, ingredient master, import/export      │
│  • Co-op light (default) + optional Co-op dark (desk)   │
│  • Data section renamed: Import / Export / Backup       │
└─────────────────────────────────────────────────────────┘
```

**Signature element (one memorable thing):** recipe cards as **index cards** — off-white surface, subtle top rule, **dept-colored left stripe** (bakery amber, deli green, prepared red, default co-op red). Everything else stays quiet.

## Token system (target)

### Palette — Kitchen Light (locked)

| Token | Hex | Use |
|-------|-----|-----|
| `paper` | `#faf8f4` | Page bg |
| `card` | `#ffffff` | Cards |
| `ink` | `#1a1614` | Body text |
| `muted` | `#6b635a` | Meta |
| `rule` | `#e8e0d4` | Borders, index lines |
| `brand` | `#b42318` | Co-op red — actions, active nav |
| `brand-soft` | `#fdecea` | Hover, selected |

### Dept stripes (semantic, not themes)

| Tag | Stripe |
|-----|--------|
| `#bakery` | `#c47a1a` |
| `#deli` / `#lunch` | `#2d6a4f` |
| `#breakfast` | `#d4a017` |
| `#dinner` | `#7b2d26` |
| default | `#b42318` |

### Typography

| Role | Face | Why |
|------|------|-----|
| Display | **Libre Baskerville** | Recipe-book authority; less "startup serif" than Fraunces |
| Body / UI | **IBM Plex Sans** | Industrial, readable at 115% on tablets; not DM Sans default |
| Data / costs | **IBM Plex Mono** | Ingredient costs, pull-list qty |

### Layout constants

- `--touch: 48px` (kitchen); `--touch: 44px` (office)
- `--radius: 8px` (cards — not 10px pill-soft)
- Max width kitchen: `720px` (single column, thumb reach)
- Max width office: `960px`

## Structural changes (nav + IA)

| Current | Proposed |
|---------|----------|
| Import (nav) | **Data** — import URL, import file, export, backup |
| More ▾ | **Plan** — production, meal plan, shopping |
| Theme ▾ (6) | Removed kitchen; office gets Light / Dark toggle only |
| Recipes intro text | Kitchen: search only, no prose |
| Tag quick filters | Keep — make chips match dept stripe colors |

## Preview all three designs

Open the design hub (requires `larder-dev` running):

**http://127.0.0.1:18080/prototypes/**

| Tab | What |
|-----|------|
| **Current live** (`?view=current`) | Shipped SPA at `/` |
| **Kitchen** (`?view=kitchen`) | Proposed floor mode |
| **Office** (`?view=office`) | Proposed manager mode |
| **Compare all** (`?view=compare`) | Three tablet frames side-by-side |

Keyboard: `1` `2` `3` `C` · Device size: Phone or Tablet dropdown.

Source files: `server/src/static/prototypes/` (also mirrored in `projects/larder/prototypes/`).

## Design toolchain

Use these in order when changing ricing:

| Tool | Path / command | Purpose |
|------|----------------|---------|
| **Design contract** | `projects/larder/DESIGN.md` | This file — tokens, modes, rules |
| **HTML prototypes** | `projects/larder/prototypes/*.html` | Try layouts without touching SPA |
| **Extracted CSS** | `server/src/static/larder.css` | Split from index.html before big refactor |
| **Token JSON** | `projects/larder/design/tokens.json` (future) | Single source for CSS + print sheets |
| **Browser DevTools** | Screenshots at tablet width (`:18080`) | Visual check after changes |
| **Print preview** | In-app modal | Verify prep/pull/shopping at 100% zoom |
| **Role smoke test** | `kitchen@` vs `manager@` | Two screenshots every visual change |
| **Tablet viewport** | 768×1024 in browser devtools | Primary target |
| **Homelab reference** | `homelab/public/food-cohesive-prototype-v2.html` | **Do not copy** — different product (personal dashboard); contrast only |

### Change workflow

1. Sketch in `prototypes/` if layout changes (not just color).
2. Update tokens in this file first.
3. Apply to `index.html` CSS (or `larder.css` once extracted).
4. Screenshot kitchen + manager at 768px.
5. Print-preview one prep sheet — confirm readable on paper.
6. Check `prefers-reduced-motion` (no new animation without opt-out).

## Motion

- **Allow:** 150ms hover lift on recipe cards; modal fade; toast slide
- **Disallow:** page-load sequences, parallax, staggered list reveals
- Kitchen mode: **zero** decorative motion

## Print (keep separate from screen theme)

Print HTML stays **white paper, black ink, Georgia** — already correct. Screen ricing must not leak into `@media print`.

## Implementation phases

| Phase | Work | Risk |
|-------|------|------|
| **D1** | Rename Import → Data; Plan submenu; dept stripe on cards | Low — HTML/CSS only |
| **D2** | `body.kitchen-mode` layout (minimal header, bigger search) | Medium |
| **D3** | Remove 6-theme picker → office Light/Dark only | Low |
| **D4** | Extract CSS to `larder.css` + `tokens.css` | Medium — refactor |
| **D5** | Typography swap (Plex), index-card surfaces | Low once tokens exist |

## Out of scope

- Matching homelab dashboard night-sky aesthetic
- Parsley/ChefTec visual parity
- Custom icon set (MDI via Homepage is enough for dashboard card)
- Dark kitchen theme (fluorescent kitchens need light)

## Decisions (locked 2026-07-23)

1. **Display serif:** Libre Baskerville
2. **Kitchen default route:** search-only home (not last-opened)
3. **Dept stripes:** derive from primary tag (existing `pick_primary_tag` priority)
4. **TUI long descriptions:** truncate with ellipsis (fixed header height)

---

*Last updated: 2026-07-23 — open decisions locked; D1–D5 runway in progress.*
