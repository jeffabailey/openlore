//! `paths` — XDG-resolved filesystem layout for OpenLore.
//!
//! Resolution rules (per DEVOPS distribution.md):
//!
//! - Config: `$XDG_CONFIG_HOME/openlore/` or `$HOME/.config/openlore/`.
//! - Data: `$XDG_DATA_HOME/openlore/` or `$HOME/.local/share/openlore/`.
//!
//! The `OPENLORE_HOME` override is recognized for tests so the
//! subprocess-driven acceptance suite can sandbox to a tempdir without
//! relying on the host's real `$HOME`. When `OPENLORE_HOME` is set, the
//! config and data dirs resolve directly under it as
//! `<OPENLORE_HOME>/.config/openlore/` and
//! `<OPENLORE_HOME>/.local/share/openlore/`.
//!
//! The two-prompt UX makes path resolution a load-bearing concern: the
//! init verb writes to the config dir, the storage adapter writes to the
//! data dir, and the user must be able to find them after the run.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

/// Resolved filesystem layout for one OpenLore process.
#[derive(Debug, Clone)]
pub struct OpenLorePaths {
    /// `$XDG_CONFIG_HOME/openlore` or fallback.
    pub config_dir: PathBuf,
    /// `$XDG_DATA_HOME/openlore` or fallback.
    pub data_dir: PathBuf,
}

impl OpenLorePaths {
    /// Resolve from the environment per the rules above. The
    /// `OPENLORE_HOME` override takes precedence over the standard XDG
    /// env vars so test sandboxing is unambiguous.
    pub fn from_env() -> Result<Self> {
        let (config_root, data_root) = if let Ok(home) = std::env::var("OPENLORE_HOME") {
            let home_path = PathBuf::from(home);
            (
                home_path.join(".config"),
                home_path.join(".local").join("share"),
            )
        } else {
            let config_root = match std::env::var("XDG_CONFIG_HOME") {
                Ok(v) if !v.is_empty() => PathBuf::from(v),
                _ => default_home()?.join(".config"),
            };
            let data_root = match std::env::var("XDG_DATA_HOME") {
                Ok(v) if !v.is_empty() => PathBuf::from(v),
                _ => default_home()?.join(".local").join("share"),
            };
            (config_root, data_root)
        };

        Ok(Self {
            config_dir: config_root.join("openlore"),
            data_dir: data_root.join("openlore"),
        })
    }

    /// Path to the identity TOML: `<config>/identity.toml`.
    pub fn identity_toml(&self) -> PathBuf {
        self.config_dir.join("identity.toml")
    }

    /// Path to the DuckDB file: `<data>/openlore.duckdb`.
    pub fn duckdb_file(&self) -> PathBuf {
        self.data_dir.join("openlore.duckdb")
    }

    /// Path to the claims artifact directory: `<data>/claims/`.
    pub fn claims_dir(&self) -> PathBuf {
        self.data_dir.join("claims")
    }

    /// Path to the minted-philosophy artifact directory:
    /// `<data>/philosophies/` (ADR-059 §4.5, slice-24). Colocated with the
    /// DuckDB file so it matches `DuckDbStorageAdapter`'s own write location.
    pub fn philosophies_dir(&self) -> PathBuf {
        self.data_dir.join("philosophies")
    }
}

fn default_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("neither OPENLORE_HOME nor HOME nor XDG_*_HOME is set"))
}
