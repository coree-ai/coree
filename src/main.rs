use anyhow::Result;
use clap::{Parser, Subcommand};
use coree::{config::Config, index, inject, remote, request, serve, status};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "coree",
    version,
    about = "Persistent memory and code intelligence for AI agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the MCP server over stdio
    Serve {
        #[arg(long, help = "Path to .coree.toml (default: auto-discover)")]
        config: Option<PathBuf>,
        #[arg(long, help = "Force full index rebuild even if versions match")]
        reindex: bool,
    },
    /// Inject memory context into agent hooks (short-lived, always exits 0)
    Inject {
        #[arg(
            long,
            default_value = "prompt",
            help = "Injection type: prompt | session | stop | compact"
        )]
        r#type: String,
        #[arg(long, help = "Explicit query string (prompt type only)")]
        query: Option<String>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long, help = "Max output bytes (default: 32000 session/compact, 8000 prompt/stop)")]
        budget: Option<usize>,
        #[arg(
            long,
            default_value_t = 400,
            help = "Socket call timeout in milliseconds (0 = no timeout)"
        )]
        socket_timeout: u64,
    },
    /// Manage remote database sync
    Remote {
        #[command(subcommand)]
        subcommand: RemoteCommand,
    },
    /// Call an MCP tool on the running coree serve instance via the local socket
    Request {
        #[arg(help = "Tool name to call")]
        tool: String,
        #[arg(help = "Tool arguments as a JSON object string (optional)")]
        args: Option<String>,
    },
    /// Show current configuration and database status
    Status,
    /// Force a full rebuild of the code index on next serve start (clears stored logic version)
    Reindex,
}

#[derive(Subcommand)]
enum RemoteCommand {
    /// Migrate local database to a remote backend and enable sync
    Enable {
        #[arg(long, help = "Remote database URL")]
        url: Option<String>,
        #[arg(long, help = "Auth token")]
        token: Option<String>,
        #[arg(long, help = "Overwrite remote database if it already has data")]
        force: bool,
    },
    /// Seed an empty remote database from the local backup
    Sync {
        #[arg(long, help = "Overwrite remote database even if it already has data")]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            config: config_path,
            reindex,
        } => {
            // init_tracing_to_file() is called inside serve::run() after log::init(),
            // so tracing output lands in the log file rather than discarded stderr.
            let cwd = std::env::current_dir()?;
            let start = config_path
                .as_deref()
                .and_then(|p| p.parent())
                .unwrap_or(&cwd);
            let config = Config::load(start)?;
            serve::run(config, reindex).await?;
        }
        Command::Inject {
            r#type,
            query,
            limit,
            budget,
            socket_timeout,
        } => {
            coree::log::init_tracing();
            if let Err(e) = inject::run(&r#type, query, limit, budget, socket_timeout).await {
                eprintln!("coree inject error: {e}");
            }
        }
        Command::Remote { subcommand } => {
            let cwd = std::env::current_dir()?;
            let config = Config::load(&cwd)?;
            match subcommand {
                RemoteCommand::Enable { url, token, force } => {
                    remote::enable(&config, url, token, force).await?;
                }
                RemoteCommand::Sync { force } => {
                    let msg = remote::sync(&config, force).await?;
                    println!("{msg}");
                }
            }
        }
        Command::Request { tool, args } => {
            coree::log::init_tracing();
            let cwd = std::env::current_dir()?;
            let config = Config::load(&cwd)?;
            if let Err(e) = request::run(&config, &tool, args.as_deref()).await {
                eprintln!("coree request error: {e}");
                std::process::exit(1);
            }
        }
        Command::Status => {
            let cwd = std::env::current_dir()?;
            let config = Config::load(&cwd)?;
            status::run(&config).await?;
        }
        Command::Reindex => {
            let cwd = std::env::current_dir()?;
            let config = Config::load(&cwd)?;
            let db_path = config.index_db_path();
            if !db_path.exists() {
                println!("No index database found at {}. Index will be built on next serve start.", db_path.display());
                return Ok(());
            }
            index::reset_stored_version(&db_path).await?;
            println!("Index version reset at {}. Restart coree serve to trigger a full reindex.", db_path.display());
        }
    }

    Ok(())
}
