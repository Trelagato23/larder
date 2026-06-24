# larder

Recipe manager with a CLI, terminal UI, and small web UI. SQLite backend, single binary.

## Build

```bash
cargo build --release
```

## Usage

```bash
larder init
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
