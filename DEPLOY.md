# Larder deployment

## Personal vs work profiles

Keep **two SQLite databases** so work tablets never see personal recipes.

| Profile | Default path | Use |
|---------|--------------|-----|
| `personal` | `~/.local/share/larder/personal.db` | Home / meal prep |
| `work` | `~/.local/share/larder/work.db` | Co-op deploy |

```bash
# Tag work recipes in your personal DB (or put them in a cookbook)
larder --database sqlite:$HOME/.local/share/larder/personal.db tag add "Protein Pitas" work

# Export only #work recipes
./scripts/profile.sh personal export-work
# → ~/.local/share/larder/work-bundle.json

# Seed / refresh the work DB, then serve it for tablets
./scripts/profile.sh work init
./scripts/profile.sh work import-bundle ~/.local/share/larder/work-bundle.json
./scripts/profile.sh work serve
```

CLI filters (any DB):

```bash
larder export --format json --tag work -o work-bundle.json
larder export --format json --cookbook "Bakery Board" -o bakery.json
```

Web (manager → **Data**): set **Tag filter** and/or cookbook before Download.

Override data directory with `LARDER_DATA_DIR`. Work tag name defaults to `work` (`LARDER_WORK_TAG`).

## Architecture (L3.3)

**One server, multi-location** — single SQLite database, shared recipe library, per-store ingredient pricing via `X-Larder-Location` header. Kitchen tablets at Elmwood and Hertel hit the same instance; managers set costs per store.

No sync between databases required for the two-store pilot.

## Quick start (local / workstation)

```bash
cd projects/larder
cargo build -p larder-server --release
export DATABASE_URL=sqlite:/path/to/larder.db
export LARDER_ADDR=0.0.0.0:18080
export LARDER_JWT_SECRET=change-me-in-production
./target/release/larder-server
```

Open `http://<host>:18080/`. Demo logins: `manager@larder.local` / `manager`, `kitchen@larder.local` / `kitchen`.

**Note:** Port 8080 may conflict with Pi-hole on some hosts — use 18080 or set `LARDER_ADDR`.

## Docker

```bash
cd projects/larder
docker compose up --build -d
```

- Data: volume `larder-data` → `/data/larder.db`
- Env: `DATABASE_URL=sqlite:/data/larder.db`, `LARDER_JWT_SECRET` (required in prod)

## Homelab (NAS / LAN tablets)

```bash
cd projects/homelab/larder
docker compose up -d --build
```

Suggested URL: `http://192.168.5.10:18080/` (add to Homepage `services.yaml`).

### Backup

```bash
cp /data/larder.db /backup/larder-$(date +%F).db
```

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | `sqlite:larder.db` | SQLite path |
| `LARDER_ADDR` / `PORT` | `0.0.0.0:8080` | Listen address |
| `LARDER_JWT_SECRET` | dev-only | **Set in production** |
| `LARDER_STATIC_DIR` | `server/src/static` | SPA path (Docker sets `/app/static`) |

## Production workflow

1. **More → Production** — create plan for today
2. Add recipes with batch counts (e.g. 3× Country Sourdough)
3. **Print pull list** — aggregated ingredients with UOM conversion
