use anyhow::{Context, Result};
use libsql::{Builder, Connection, Database};
use std::path::Path;

use crate::config::{BackendMode, Config};

pub struct Db {
    pub conn: Connection,
    _db: Database,
}

impl Db {
    pub async fn open(config: &Config) -> Result<Self> {
        let db = match config.backend.mode {
            BackendMode::Local => {
                let path = config.local_db_path();
                ensure_parent_dir(&path)?;
                Builder::new_local(&path)
                    .build()
                    .await
                    .with_context(|| format!("Failed to open local DB at {}", path.display()))?
            }
            BackendMode::Replica => {
                let path = config.local_db_path();
                ensure_parent_dir(&path)?;
                let url = config
                    .backend
                    .remote_url
                    .as_deref()
                    .context("replica mode requires backend.remote_url")?;
                let token = config
                    .backend
                    .auth_token
                    .as_deref()
                    .context("replica mode requires backend.auth_token")?;
                Builder::new_remote_replica(path.to_str().unwrap(), url.to_string(), token.to_string())
                    .sync_interval(std::time::Duration::from_secs(1))
                    .build()
                    .await
                    .with_context(|| format!("Failed to open replica DB at {}", path.display()))?
            }
        };

        let conn = db.connect().context("Failed to connect to database")?;
        Ok(Self { conn, _db: db })
    }
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}
