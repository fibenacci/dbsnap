# dbsnap

> Deterministic database snapshots, diffs and integrity verification for modern systems.

dbsnap is to relational database *state* what Git is to source code: reproducible
snapshots, semantic diffs, an immutable hash chain, and integrity verification.
It does **not** replace backups — it makes database state inspectable, verifiable
and replayable.

## Status

MVP — PostgreSQL only. Implements the full first-release scope:
deterministic snapshots, commit manifests, row & table hashing, semantic diffs,
integrity verification, and export.

## Install / build

```bash
cargo build --release
# binary at target/release/dbsnap
```

## Quick start

```bash
# 1. Spin up a throwaway Postgres (optional)
docker compose up -d
export DATABASE_URL=postgres://dbsnap:dbsnap@localhost:5433/dbsnap

# 2. Initialize a repository (creates ./.dbsnap)
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

## Workspace layout

| Crate | Responsibility |
|---|---|
| `dbsnap-hashing` | BLAKE3, domain separation, canonical JSON hashing |
| `dbsnap-core` | Domain model + the deterministic hash hierarchy |
| `dbsnap-postgres` | Schema introspection & row capture via `to_jsonb` |
| `dbsnap-storage` | Content-addressed filesystem store + ref resolution |
| `dbsnap-diff` | Semantic diff engine (insert/update/delete + column changes) |
| `dbsnap-integrity` | Hash-chain verification |
| `dbsnap-export` | JSON / SQL export of historical state |
| `dbsnap-cli` | `dbsnap` binary (clap + tokio) |

## Known MVP limitations

- Whole-table capture loads rows into memory (no streaming yet).
- Tables without a primary key are keyed by full-row content; identical
  duplicate rows collapse to one.
- `NUMERIC` fidelity relies on `serde_json` `arbitrary_precision` (lossless).

## License

MIT OR Apache-2.0
