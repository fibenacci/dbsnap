# Contributing to dbsnap

## Commit convention

We use [Conventional Commits](https://www.conventionalcommits.org/) **with a
leading emoji per type**. The format is:

```
<emoji> <type>(<scope>)<!>: <subject>
```

- `<scope>` is optional (e.g. the crate or area: `report`, `mysql`, `engine`).
- A trailing `!` (or a `BREAKING CHANGE:` footer) marks a breaking change and
  triggers a major version bump.
- Keep the subject in the imperative mood, lower-case, no trailing period.

### Type → emoji

| Type       | Emoji | Use for                                   |
|------------|:-----:|-------------------------------------------|
| `feat`     | ✨    | a new feature                             |
| `fix`      | 🐛    | a bug fix                                 |
| `perf`     | ⚡    | a performance improvement                 |
| `refactor` | ♻️    | code change that neither fixes nor adds   |
| `docs`     | 📝    | documentation only                        |
| `test`     | ✅    | adding or fixing tests                    |
| `build`    | 📦    | build system or dependencies              |
| `ci`       | 👷    | CI configuration                          |
| `style`    | 🎨    | formatting / style, no logic change       |
| `chore`    | 🔧    | tooling / housekeeping                    |
| `revert`   | ⏪    | reverting a previous commit               |

### Examples

```
✨ feat(report): add self-contained HTML report command
🐛 fix(mysql): cast information_schema columns to CHAR to avoid VARBINARY decode
♻️ refactor: harden DB introspection with try_get and shrink the driver API
📝 docs: add roadmap and release strategy
```

The changelog groups entries by type under these emoji headings automatically
(see `release-plz.toml`); the leading emoji is stripped before the
Conventional-Commit type is parsed, so both stay consistent.

## Releases

Releasing is automated and split between two tools:

- **[release-plz](https://release-plz.dev/)** — a job in the CI workflow that,
  on every push to `main` (after the CI gates pass), opens a "release" PR that
  bumps the version and updates `CHANGELOG.md` from the commits. Merging that PR
  pushes a `v<version>` git tag. It does **not** publish
  to crates.io (dbsnap is an application) and does **not** create the GitHub
  release.
- **[cargo-dist](https://opensource.axo.dev/cargo-dist/)** — triggered by the
  `v*` tag, builds the `dbsnap` binary for all targets, generates installers and
  checksums, and creates the GitHub release with those artifacts.

So the flow is: merge feature PRs → release-plz opens a release PR → merge it →
tag → cargo-dist builds & publishes the release.

### Local checks

Run the full pipeline locally before pushing:

```bash
make ci          # fmt + clippy + MSRV check + docs + deny + tests
```
