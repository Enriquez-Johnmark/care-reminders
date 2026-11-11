//! The `care-reminders` command.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use care_reminders::engine::accounts::Accounts;
use care_reminders::engine::clock::SystemClock;
use care_reminders::fhir_server::{FhirServers, ServerAddress};
use care_reminders::records::ClinicId;

const USAGE: &str = "usage: care-reminders migrate | demo-data | clinic add --name NAME --email EMAIL | \
                     clinic set-fhir-server --clinic ID --url URL | fetch --clinic ID | load-made-up --url URL | \
                     work | evidence --commit SHA [--signed true|false] [--junit FILE] [--out FILE]";

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
            let pool = connect().await?;
            let accounts = Accounts::new(pool.clone(), Arc::new(SystemClock));
            let clinic = accounts.create(name, email).await.context("adding the clinic")?;
            care_reminders::rules::Rules::new(pool, Arc::new(SystemClock))
                .install_defaults(clinic, "operator")
                .await
                .context("giving the clinic its default rules")?;
            println!("{clinic}");
            Ok(())
        }
        Some("clinic") if args.get(1).map(String::as_str) == Some("set-fhir-server") => {
            let clinic = clinic_option(&args)?;
            let address = ServerAddress::parse(option(&args, "--url")?)?;
            FhirServers::new(connect().await?, Arc::new(SystemClock))
                .set(clinic, &address, "operator")
                .await
                .context("setting the FHIR server")?;
            println!("{clinic} fetches from {address}");
            Ok(())
        }
        Some("fetch") => {
            let clinic = clinic_option(&args)?;
            let outcome = FhirServers::new(connect().await?, Arc::new(SystemClock))
                .fetch(clinic, "operator")
                .await?;
            println!(
                "from {}: {} new, {} updated, {} unchanged, {} refused",
                outcome.server,
                outcome.new,
                outcome.updated,
                outcome.unchanged,
                outcome.refused.len()
            );
            for refused in &outcome.refused {
                println!("  {}: {}", refused.patient, refused.reason);
            }
            Ok(())
        }
        Some("load-made-up") => {
            let address = ServerAddress::parse(option(&args, "--url")?)?;
            let today = care_reminders::engine::clock::Clock::now(&SystemClock).date();
            let loaded = care_reminders::fhir_server::load_made_up(&address, today).await?;
            println!("loaded {loaded} made-up patients into {address}");
            Ok(())
        }
        Some("work") => work().await,
        Some("demo-data") => write_demo_data(),
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

/// Writes the made-up patients, made for the demo day, to `demo/patients/`.
fn write_demo_data() -> anyhow::Result<()> {
    use care_reminders::demo_data::{DEMO_DAY, DIR, generate};
    let dir = std::path::Path::new(DIR);
    std::fs::create_dir_all(dir).context("creating the demo folder")?;
    let files = generate(DEMO_DAY);
    for file in &files {
        std::fs::write(dir.join(&file.name), &file.json).with_context(|| format!("writing {}", file.name))?;
    }
    println!("wrote {} made-up patients to {}", files.len(), dir.display());
    Ok(())
}

/// The clinic named by `--clinic`.
fn clinic_option(args: &[String]) -> anyhow::Result<ClinicId> {
    Ok(option(args, "--clinic")?
        .parse::<uuid::Uuid>()
        .context("--clinic is not a clinic ID")?
        .into())
}

/// The background worker: every minute, the hourly fetch for each clinic
/// that is due (H2).
async fn work() -> anyhow::Result<()> {
    let servers = FhirServers::new(connect().await?, Arc::new(SystemClock));
    loop {
        for (clinic, result) in servers.fetch_due().await? {
            if let Err(err) = result {
                eprintln!("care-reminders: the hourly fetch for {clinic} failed: {err}");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
