# Larder

A kitchen recipe book in the browser. Look up a recipe, scale a batch, and print a prep sheet — no app to install. Kitchen never sees food cost.

Bookmark one address on a store PC. If the machine is down, use paper, like today.

## What it does

**Kitchen**
- Search by name, description, or ingredient
- Filter by station (bakery, hot bar, grab & go, soups, sandwiches, …)
- Scale servings or batches; weights and volume where we have them
- Print / view a high-contrast prep sheet (allergens on the sheet)
- Cook mode with step timers
- PLU lookup, temps / reference, converter and timer

**Manager**
- Edit recipes (qty, unit, name, steps, yield, allergens, station)
- Shared ingredient list — change a cost, linked recipes update
- Import / export / database backup

The current work library is about 1,100 recipes converted from ChefTec. A fresh export would finish the book.

## Install (small)

1. Copy one small program and the recipe file onto a PC that already stays on. Bookmark it.
2. Docker, only if you already use it — one compose file, same bookmark.
3. Office PC + print until there is a screen in the kitchen.

No extra services. No cloud account. Kitchen only uses the bookmark.

### Run the program

```bash
git clone https://github.com/Trelagato23/larder.git
cd larder
cargo build -p larder-server --release
export DATABASE_URL=sqlite:larder.db
export LARDER_JWT_SECRET=change-me-in-production
./target/release/larder-server
```

Open `http://localhost:8080/` (or set `LARDER_ADDR` / `PORT`). Kitchen and manager are separate logins; set real accounts and a real JWT secret before a store install.

### Docker

```bash
docker compose up --build
```

The database file is `/data/larder.db` in the volume. Details: [DEPLOY.md](DEPLOY.md).

## Also in this repo

CLI and a terminal UI live under `tui/`. The kitchen bookmark is the web app in `server/`.

```
core/     models, SQLite, import / export
server/   web app
tui/      terminal UI and CLI
```

## License

AGPL-3.0
