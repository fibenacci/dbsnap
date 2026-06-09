# dbsnap

> Deterministic database snapshots, diffs and integrity verification for modern systems.

dbsnap is to relational database *state* what Git is to source code: reproducible
snapshots, semantic diffs, an immutable hash chain, and integrity verification.
It does **not** replace backups — it makes database state inspectable, verifiable
and replayable.

## Status

MVP — **PostgreSQL** and **MySQL / MariaDB**. Implements the full first-release
scope: deterministic snapshots, commit manifests, row & table hashing, semantic
diffs, integrity verification, and export. The engine is selected automatically
from the connection string's URL scheme.

## Install / build

Tagged releases ship prebuilt `dbsnap` binaries for Linux/macOS/Windows plus a
shell/PowerShell installer on the [GitHub Releases page](https://github.com/fibenacci/dbsnap/releases)
(built by cargo-dist). To build from source:

```bash
cargo build --release
# binary at target/release/dbsnap
```

## Quick start

```bash
# 1. Spin up a throwaway database (optional)
docker compose up -d
export DATABASE_URL=postgres://dbsnap:dbsnap@localhost:5433/dbsnap
# ...or MySQL:
# export DATABASE_URL=mysql://dbsnap:dbsnap@localhost:3307/dbsnap

# 2. Initialize a repository (creates ./.dbsnap)
#    For MySQL, set --schema to the database name (e.g. --schema dbsnap).
dbsnap init

# 3. Capture state
dbsnap commit -m "before plugin install"
#    ... make changes to the database ...
dbsnap commit -m "after plugin install"

# 4. Inspect
dbsnap log
dbsnap diff HEAD~1 HEAD            # semantic diff
dbsnap diff --verbose              # show changed column values
dbsnap status                      # HEAD + uncommitted live changes

# 5. Verify integrity
dbsnap verify                      # recompute & check the stored hash chain
dbsnap verify --live               # also detect out-of-band DB mutations vs HEAD

# 6. Export historical state
dbsnap export --commit HEAD~1 --format sql
dbsnap export --format json --table public.product
```

## How it works

A Git-style Merkle hierarchy gives both **determinism** and **tamper evidence**:

```
row hash    = H(canonical_json(row))               # via Postgres to_jsonb()
table hash  = H(schema_hash, [(pk, row_hash) …])   # rows sorted by primary key
tree hash   = H([(table, table_hash, …) …])        # the whole DB state
commit hash = H(tree, parent, message, time, author)
```

Row/table/tree hashes depend only on database state, so identical state always
produces identical hashes. Commits fold in the parent hash, so altering any
ancestor changes every descendant — `dbsnap verify` recomputes the chain and
flags any mismatch.

Storage is a content-addressed, append-only `.dbsnap/` directory; unchanged
tables are stored once and shared across commits.

## When to use it

dbsnap has two distinct runtimes with different value. Don't confuse them.

### In CI/CD — measure what a *change* does to a controlled database

You do **not** compare two production databases in CI. You start from a known,
**ephemeral** database (a fixture, a seed, or a sanitized dump) and measure —
deterministically — what a change does to it. The "two databases" are two
points in time of the *same* throwaway DB, or your CI result versus a committed
golden snapshot.

- **Migration guard** — the headline case. Snapshot before, run the migration,
  snapshot after, and assert on the diff:

  ```bash
  dbsnap commit -m "before"
  bin/console database:migrate          # Doctrine / Flyway / Diesel / …
  dbsnap commit -m "after"
  dbsnap diff --json                    # assert: only expected tables changed,
                                        # e.g. no rows in payment_transaction
  ```

  Catches migrations with unintended data side-effects (a backfill that corrupts
  rows, an accidental column rewrite).

- **Schema-drift check** — compare the schema hashes a branch's migrations
  produce against a committed baseline; fail if the schema drifted unexpectedly.
- **Reproducibility** — run the migration chain twice (or `up → down → up`) and
  assert the state hash is identical; catches non-deterministic migrations.
- **Golden-snapshot tests** — like Jest snapshots, but for database state: seed
  a known input, run an import / ETL job / plugin install, and diff the result
  against a committed golden snapshot.

### In operations / incident response — integrity over time on a *real* database

- `dbsnap verify --live` detects out-of-band mutations of the live database
  versus a recorded snapshot (tamper / unauthorized-change detection).
- `dbsnap export` / replay reconstructs historical state for incident analysis.

These run against staging/production over time — **not** in CI. Plain
`dbsnap verify` (stored-chain integrity) is weak as a CI gate; the real CI value
is the **diff / migration-guard** pattern above.

## Workspace layout

| Crate | Responsibility |
|---|---|
| `dbsnap-hashing` | BLAKE3, domain separation, canonical JSON hashing |
| `dbsnap-core` | Domain model + the deterministic hash hierarchy |
| `dbsnap-postgres` | PostgreSQL introspection & row capture via `to_jsonb` |
| `dbsnap-mysql` | MySQL / MariaDB introspection & row capture via `JSON_OBJECT` |
| `dbsnap-storage` | Content-addressed filesystem store + ref resolution |
| `dbsnap-diff` | Semantic diff engine (insert/update/delete + column changes) |
| `dbsnap-integrity` | Hash-chain verification |
| `dbsnap-export` | JSON / SQL export of historical state |
| `dbsnap-cli` | `dbsnap` binary (clap + tokio) |

## Database engines

| Scheme | Engine | Status |
|---|---|---|
| `postgres://` / `postgresql://` | PostgreSQL | supported |
| `mysql://` / `mariadb://` | MySQL / MariaDB | supported |
| `sqlite://` | SQLite | planned |

The engine is selected at runtime from the connection string's URL scheme. The
whole stack below the CLI is engine-agnostic: it depends only on the
`SnapshotSource` trait (`dbsnap-core`), and the CLI's `source` registry is the
single place that maps a scheme to a concrete backend. Adding an engine is a
drop-in — a new `dbsnap-<engine>` crate implementing `SnapshotSource`, plus one
match arm in the registry. Unsupported schemes fail fast with a clear message.

Notes:

- **Determinism is per engine.** The same logical data in PostgreSQL and MySQL
  hashes differently because engines render types differently (e.g. a boolean
  is `true`/`false` in Postgres but `1`/`0` in MySQL). Cross-engine diffing
  would need a normalization layer and is out of scope for now.
- **`export --format sql` emits PostgreSQL dialect** (double-quoted identifiers,
  `::jsonb`). For MySQL, use `--format json` (engine-neutral) until a
  dialect-aware SQL exporter lands.
- **MySQL over TLS recommended.** dbsnap only ever uses the server's *public*
  key to encrypt its own password during non-TLS `caching_sha2_password` auth;
  connecting with `?ssl-mode=REQUIRED` avoids that path entirely. See the
  security note below.

### Security note (RUSTSEC-2023-0071)

Enabling MySQL pulls in the `rsa` crate (via `sqlx-mysql`), which carries
RUSTSEC-2023-0071 (the "Marvin attack" RSA timing side-channel). This advisory
concerns **private-key decryption** timing. dbsnap holds no RSA private key and
only performs **public-key encryption** of its own password during MySQL auth —
the vulnerable code path is not reachable from dbsnap, and the decrypting party
is the MySQL server, not this tool. There is no upstream fix; the issue is not
exploitable in dbsnap's usage.

## Known MVP limitations

- Whole-table capture loads rows into memory (no streaming yet).
- Tables without a primary key are keyed by full-row content; identical
  duplicate rows collapse to one.
- `NUMERIC` fidelity relies on `serde_json` `arbitrary_precision` (lossless).

## License

MIT OR Apache-2.0
