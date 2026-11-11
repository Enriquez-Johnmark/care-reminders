//! The `care-reminders` command.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use care_reminders::engine::accounts::Accounts;
use care_reminders::engine::clock::SystemClock;

const USAGE: &str = "usage: care-reminders migrate | clinic add --name NAME --email EMAIL";

fn main() -> ExitCode {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")
        .and_then(|runtime| runtime.block_on(run()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("care-reminders: {err:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("migrate") => {
            let pool = connect().await?;
            care_reminders::MIGRATOR
                .run(&pool)
                .await
                .context("running migrations")?;
            println!("migrations applied");
            Ok(())
        }
        Some("clinic") if args.get(1).map(String::as_str) == Some("add") => {
            let (name, email) = (option(&args, "--name")?, option(&args, "--email")?);
            let accounts = Accounts::new(connect().await?, Arc::new(SystemClock));
            let clinic = accounts.create(name, email).await.context("adding the clinic")?;
            println!("{clinic}");
            Ok(())
        }
        _ => anyhow::bail!(USAGE),
    }
}

async fn connect() -> anyhow::Result<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    sqlx::PgPool::connect(&url).await.context("connecting to the database")
}

/// The value after `name` in `args`.
fn option<'a>(args: &'a [String], name: &str) -> anyhow::Result<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1))
        .map(String::as_str)
        .with_context(|| format!("{name} is missing"))
}
