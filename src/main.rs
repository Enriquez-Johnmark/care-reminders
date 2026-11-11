//! The `care-reminders` command.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use care_reminders::engine::accounts::Accounts;
use care_reminders::engine::clock::SystemClock;

const USAGE: &str = "usage: care-reminders migrate | clinic add --name NAME --email EMAIL | \
                     evidence --commit SHA [--signed true|false] [--junit FILE] [--out FILE]";

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
        Some("evidence") => evidence(&args),
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

/// Writes the evidence pack and fails if a finished milestone has a gap (J1).
fn evidence(args: &[String]) -> anyhow::Result<()> {
    let read = |path: &str| std::fs::read_to_string(path).with_context(|| format!("reading {path}"));
    let junit = option(args, "--junit").unwrap_or("target/nextest/ci/junit.xml");
    let out = std::path::Path::new(option(args, "--out").unwrap_or("target/evidence/index.html"));
    let signed = match option(args, "--signed").ok() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let evidence = care_reminders::evidence::Evidence::build(
        &read("docs/product/spec.md")?,
        &read("docs/architecture.md")?,
        &read(junit)?,
    )?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).context("creating the evidence folder")?;
    }
    std::fs::write(out, evidence.html(option(args, "--commit")?, signed)).context("writing the evidence page")?;
    println!("wrote {}", out.display());
    let gaps = evidence.gaps();
    anyhow::ensure!(gaps.is_empty(), "{} gap(s):\n{}", gaps.len(), gaps.join("\n"));
    Ok(())
}
