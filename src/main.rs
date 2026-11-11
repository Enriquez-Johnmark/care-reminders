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
                     work [--recorded] | evidence --commit SHA [--signed true|false] [--junit FILE] [--out FILE]";

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
                "from {}: {} new, {} updated, {} unchanged, {} refused; {} follow-ups chosen, {} stopped as booked",
                outcome.server,
                outcome.new,
                outcome.updated,
                outcome.unchanged,
                outcome.refused.len(),
                outcome.follow_ups_chosen,
                outcome.follow_ups_booked
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
        Some("work") => work(&args).await,
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
/// that is due (H2), then drafting each reminder that is due (E6). Drafts are
/// worded by the live model with the owner's key, by recorded answers with
/// `--recorded`, or by the clinic's template when there is neither.
async fn work(args: &[String]) -> anyhow::Result<()> {
    use care_reminders::model::{DIR, Recorded};
    let pool = connect().await?;
    if args.iter().any(|arg| arg == "--recorded") {
        let model = Recorded::new(&drafting_model(), std::path::Path::new(DIR));
        work_with(pool, Some(model)).await
    } else {
        work_with(pool, live_model()?).await
    }
}

async fn work_with<M: care_reminders::engine::llm::LanguageModel>(
    pool: sqlx::PgPool,
    model: Option<M>,
) -> anyhow::Result<()> {
    let clock: Arc<dyn care_reminders::engine::clock::Clock> = Arc::new(SystemClock);
    let servers = FhirServers::new(pool.clone(), clock.clone());
    let drafter = care_reminders::reminders::Drafter::new(pool, clock, model);
    loop {
        match servers.fetch_due().await {
            Ok(done) => {
                for (clinic, result) in done {
                    if let Err(err) = result {
                        eprintln!("care-reminders: the hourly fetch for {clinic} failed: {err}");
                    }
                }
            }
            Err(err) => eprintln!("care-reminders: the hourly fetch failed: {err}"),
        }
        // A failed pass is reported and tried again; it never stops the worker.
        match drafter.draft_due().await {
            Ok(0) => {}
            Ok(drafted) => println!("drafted {drafted} reminder(s)"),
            Err(err) => eprintln!("care-reminders: drafting failed: {err}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

/// The model that words the drafts (`LLM_MODEL`); Sonnet unless set.
fn drafting_model() -> String {
    std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-5-5".to_owned())
}

/// The live model, with the owner's key (`OWNER_ANTHROPIC_API_KEY`, never
/// `ANTHROPIC_API_KEY`, which Claude Code itself reads), if it is set.
fn live_model() -> anyhow::Result<Option<care_reminders::engine::llm::AnthropicClient>> {
    let Some(key) = std::env::var("OWNER_ANTHROPIC_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
    else {
        return Ok(None);
    };
    let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_owned());
    let client = care_reminders::engine::llm::AnthropicClient::new(
        &base,
        key.into(),
        &drafting_model(),
        std::time::Duration::from_secs(90),
    )
    .context("setting up the model client")?;
    Ok(Some(client))
}
