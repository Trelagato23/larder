# larder

Recipe manager with a CLI, terminal UI, and small web UI. SQLite backend, single binary.

## Build

Clone the repo, then from its root:

```bash
git clone https://github.com/Trelagato23/larder.git
cd larder
cargo install --path tui --bin larder --bin larder-tui
cargo install --path server --bin larder-server
```

`larder tui` and `larder serve` spawn the other binaries — all three need to be installed (or on your `PATH`).

Or build without installing:

```bash
cargo build --release
./target/release/larder init
```

The binary lands in `~/.cargo/bin` — make sure that is on your `PATH`.

## Usage

```bash
larder init
```

A fresh database gets four starter recipes (breakfast, lunch, dinner, snack) on today's meal plan.

```bash
larder import https://example.com/some-recipe
larder list
larder search chicken
larder show <id-or-name>
larder cook <id-or-name>
larder mealplan
larder mealplan --generate    # shopping list from this week's plan
larder shopping
larder tag add <recipe> <tag>
larder export --format json
larder backup
larder tui
larder serve
```

Web UI supports edit/delete, cooking mode with timers, serving scale, export, and a mobile-friendly meal plan.

Default database: `larder.db` in the working directory. Override with `--database sqlite:/path/to/db` or `DATABASE_URL`.

Web server listens on `0.0.0.0:8080` unless `LARDER_ADDR` or `PORT` is set.

## Docker

```bash
docker compose up --build
```

Data volume mounts at `/data/larder.db`.

## Layout

- `core/` — models, SQLite, import/export, business logic
- `server/` — Axum API + static web UI
- `tui/` — Ratatui interface and CLI entrypoint

## License

AGPL-3.0
