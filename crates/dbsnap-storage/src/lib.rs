//! Content-addressed, append-only filesystem store (the `.dbsnap/` directory).
//!
//! ```text
//! .dbsnap/
//!   config.toml              connection + schema settings
//!   refs/HEAD                hex hash of the current commit
//!   commits/<hash>.json      commit manifests (keyed by commit hash)
//!   trees/<hash>.json        snapshot trees (keyed by tree hash)
//!   tables/<hash>.zst        zstd-compressed table snapshots (keyed by table hash)
//! ```
//!
//! Because tables are keyed by their content hash, an unchanged table between
//! commits is stored exactly once — commits naturally deduplicate.

use anyhow::{bail, Context, Result};
use dbsnap_core::{Commit, TableSnapshot, Tree};
use dbsnap_hashing::DbHash;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DIR: &str = ".dbsnap";
const ZSTD_LEVEL: i32 = 3;

/// Persistent configuration stored in `.dbsnap/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Schema to snapshot (PostgreSQL `search_path` schema, default `public`).
    pub schema: String,
    /// Optional connection string. The `DATABASE_URL` env var, if set,
    /// overrides this at runtime so secrets need not live on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema: "public".into(),
            database_url: None,
        }
    }
}

/// Handle to an opened repository.
pub struct Store {
    /// Path to the `.dbsnap` directory itself.
    pub root: PathBuf,
    pub config: Config,
}

impl Store {
    /// Create a fresh repository under `parent/.dbsnap`.
    pub fn init(parent: &Path, config: Config) -> Result<Store> {
        let root = parent.join(DIR);
        if root.exists() {
            bail!("{} already exists", root.display());
        }
        fs::create_dir_all(root.join("commits"))?;
        fs::create_dir_all(root.join("trees"))?;
        fs::create_dir_all(root.join("tables"))?;
        fs::create_dir_all(root.join("refs"))?;
        fs::write(root.join("config.toml"), toml::to_string_pretty(&config)?)
            .context("writing config.toml")?;
        Ok(Store { root, config })
    }

    /// Open an existing repository given its `.dbsnap` dir or its parent.
    pub fn open(path: &Path) -> Result<Store> {
        let root = if path.ends_with(DIR) {
            path.to_path_buf()
        } else {
            path.join(DIR)
        };
        let cfg_path = root.join("config.toml");
        let cfg: Config = toml::from_str(
            &fs::read_to_string(&cfg_path)
                .with_context(|| format!("reading {}", cfg_path.display()))?,
        )?;
        Ok(Store { root, config: cfg })
    }

    /// Walk up from `start` looking for a `.dbsnap` directory (like `git`).
    pub fn discover(start: &Path) -> Result<Store> {
        let mut cur = Some(start);
        while let Some(dir) = cur {
            if dir.join(DIR).join("config.toml").is_file() {
                return Store::open(&dir.join(DIR));
            }
            cur = dir.parent();
        }
        bail!("not a dbsnap repository (no {DIR} directory found); run `dbsnap init` first")
    }

    // ----- object storage -------------------------------------------------

    pub fn write_table_snapshot(&self, snap: &TableSnapshot) -> Result<DbHash> {
        let hash = snap.table_hash();
        let path = self.root.join("tables").join(format!("{hash}.zst"));
        if !path.exists() {
            let json = serde_json::to_vec(snap)?;
            let compressed = zstd::encode_all(&json[..], ZSTD_LEVEL)?;
            fs::write(&path, compressed).with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(hash)
    }

    pub fn read_table_snapshot(&self, hash: &DbHash) -> Result<TableSnapshot> {
        let path = self.root.join("tables").join(format!("{hash}.zst"));
        let compressed =
            fs::read(&path).with_context(|| format!("reading table object {}", path.display()))?;
        let json = zstd::decode_all(&compressed[..])?;
        Ok(serde_json::from_slice(&json)?)
    }

    pub fn write_tree(&self, tree: &Tree) -> Result<DbHash> {
        let hash = tree.hash();
        let path = self.root.join("trees").join(format!("{hash}.json"));
        if !path.exists() {
            fs::write(&path, serde_json::to_vec_pretty(tree)?)?;
        }
        Ok(hash)
    }

    pub fn read_tree(&self, hash: &DbHash) -> Result<Tree> {
        let path = self.root.join("trees").join(format!("{hash}.json"));
        let bytes =
            fs::read(&path).with_context(|| format!("reading tree object {}", path.display()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn write_commit(&self, commit: &Commit) -> Result<DbHash> {
        let hash = commit.hash();
        let path = self.root.join("commits").join(format!("{hash}.json"));
        if !path.exists() {
            fs::write(&path, serde_json::to_vec_pretty(commit)?)?;
        }
        Ok(hash)
    }

    pub fn read_commit(&self, hash: &DbHash) -> Result<Commit> {
        let path = self.root.join("commits").join(format!("{hash}.json"));
        let bytes =
            fs::read(&path).with_context(|| format!("reading commit object {}", path.display()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    // ----- refs -----------------------------------------------------------

    pub fn set_head(&self, hash: &DbHash) -> Result<()> {
        fs::write(self.root.join("refs").join("HEAD"), hash.to_hex())?;
        Ok(())
    }

    pub fn head(&self) -> Result<Option<DbHash>> {
        let path = self.root.join("refs").join("HEAD");
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(DbHash::from_str(text).context("parsing HEAD")?))
    }

    // ----- ref resolution -------------------------------------------------

    /// Resolve a reference to a commit hash.
    ///
    /// Supported forms: `HEAD`, `HEAD~N`, a full 64-char hash, or an
    /// unambiguous hash prefix.
    pub fn resolve(&self, reference: &str) -> Result<DbHash> {
        let reference = reference.trim();

        if let Some(rest) = reference.strip_prefix("HEAD") {
            let head = self
                .head()?
                .context("HEAD does not exist yet (no commits)")?;
            if rest.is_empty() {
                return Ok(head);
            }
            let n: usize = rest
                .strip_prefix('~')
                .and_then(|d| d.parse().ok())
                .with_context(|| format!("invalid reference '{reference}'"))?;
            let mut cur = head;
            for step in 0..n {
                let commit = self.read_commit(&cur)?;
                cur = commit.parent.with_context(|| {
                    format!(
                        "reference '{reference}' goes past the root commit (only {step} parents)"
                    )
                })?;
            }
            return Ok(cur);
        }

        // Full hash.
        if let Ok(h) = DbHash::from_str(reference) {
            return Ok(h);
        }

        // Hash prefix lookup against the commits directory.
        self.resolve_prefix(reference)
    }

    fn resolve_prefix(&self, prefix: &str) -> Result<DbHash> {
        if prefix.len() < 4 {
            bail!("ambiguous reference '{prefix}': use at least 4 hex characters");
        }
        let mut matches = Vec::new();
        for entry in fs::read_dir(self.root.join("commits"))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".json") {
                if stem.starts_with(prefix) {
                    matches.push(stem.to_string());
                }
            }
        }
        match matches.len() {
            0 => bail!("no commit matches '{prefix}'"),
            1 => Ok(DbHash::from_str(&matches[0])?),
            _ => bail!(
                "ambiguous reference '{prefix}' matches {} commits",
                matches.len()
            ),
        }
    }

    /// Return the commit chain from `from` back to the root (newest first).
    pub fn chain(&self, from: &DbHash) -> Result<Vec<(DbHash, Commit)>> {
        let mut out = Vec::new();
        let mut cur = Some(*from);
        while let Some(hash) = cur {
            let commit = self.read_commit(&hash)?;
            cur = commit.parent;
            out.push((hash, commit));
        }
        Ok(out)
    }
}
