//! The `care-reminders` command.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use care_reminders::engine::accounts::Accounts;
use care_reminders::engine::clock::SystemClock;
use care_reminders::fhir_server::{FhirServers, ServerAddress};
use care_reminders::records::ClinicId;

const USAGE: &str = "usage: care-reminders migrate | demo-data | clinic add --name NAME --email EMAIL | \
                     clinic add-coordinator --clinic ID --email EMAIL --name NAME | \
                     clinic set-phone --clinic ID --phone PHONE | \
                     clinic set-fhir-server --clinic ID --url URL | fetch --clinic ID | load-made-up --url URL | \
                     work [--recorded] | serve [--recorded] | \
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
        Some("clinic") if args.get(1).map(String::as_str) == Some("add-coordinator") => {
            let clinic = clinic_option(&args)?;
            let (email, name) = (option(&args, "--email")?, option(&args, "--name")?);
            care_reminders::review::Reviews::new(connect().await?, Arc::new(SystemClock))
                .add_coordinator(clinic, email, name, "operator")
                .await
                .context("adding the coordinator")?;
            println!("added {name} <{email}>");
            Ok(())
        }
        Some("clinic") if args.get(1).map(String::as_str) == Some("set-phone") => {
            let clinic = clinic_option(&args)?;
            let phone = option(&args, "--phone")?;
            care_reminders::settings::set_phone(&connect().await?, clinic, phone).await?;
            println!("{clinic}'s reminders give {phone}");
            Ok(())
        }
        Some("serve") => serve(args.iter().any(|arg| arg == "--recorded")).await,
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
    let model = choose_model(args.iter().any(|arg| arg == "--recorded"))?;
    let clock: Arc<dyn care_reminders::engine::clock::Clock> = Arc::new(SystemClock);
    run_worker(connect().await?, clock, model).await;
    Ok(())
}

/// Recorded answers, the live model with the owner's key, or none.
fn choose_model(recorded: bool) -> anyhow::Result<Option<care_reminders::model::AnyModel>> {
    use care_reminders::model::{AnyModel, DIR, Recorded};
    Ok(if recorded {
        Some(AnyModel::Recorded(Recorded::new(
            &drafting_model(),
            std::path::Path::new(DIR),
        )))
    } else {
        live_model()?.map(AnyModel::Live)
    })
}

/// Runs the fetch and drafting sweeps every minute, for good. A failed pass
/// is reported and tried again; it never stops the worker.
async fn run_worker(
    pool: sqlx::PgPool,
    clock: Arc<dyn care_reminders::engine::clock::Clock>,
    model: Option<care_reminders::model::AnyModel>,
) {
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
        match drafter.draft_due().await {
            Ok(0) => {}
            Ok(drafted) => println!("drafted {drafted} reminder(s)"),
            Err(err) => eprintln!("care-reminders: drafting failed: {err}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

/// How often `serve` drains the outbox when nothing wakes it.
const TICK: std::time::Duration = std::time::Duration::from_secs(15);

/// Runs the web app, the outbox sender (login emails go out at once) and the
/// worker, until stopped. Development settings by default: the app on
/// 127.0.0.1:3000 and email caught by Mailpit.
async fn serve(recorded: bool) -> anyhow::Result<()> {
    use care_reminders::engine::mail::MailpitMailer;
    use care_reminders::engine::outbox::{NoExtraChecks, Sender};
    use care_reminders::web::{AppState, router};

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let setting = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
    let bind = setting("BIND_ADDR", "127.0.0.1:3000");
    let base_url = setting("PUBLIC_BASE_URL", &format!("http://{bind}"));
    let pool = connect().await?;
    care_reminders::MIGRATOR
        .run(&pool)
        .await
        .context("running migrations")?;
    let clock: Arc<dyn care_reminders::engine::clock::Clock> = Arc::new(SystemClock);

    let mut state = AppState::new(pool.clone(), clock.clone(), &base_url);
    if let Some(model) = choose_model(recorded)? {
        state = state.with_model(model);
    }
    if let Ok(dir) = std::env::var("WEB_DIR") {
        state = state.with_web_dir(std::path::Path::new(&dir));
    }
    let wake = state.sender_wake_up();
    let mailer = MailpitMailer::new(
        &setting("MAILPIT_URL", "http://localhost:8025"),
        &setting("MAIL_FROM_ADDRESS", "reminders@example-family-practice.test"),
    )
    .context("setting up Mailpit")?;
    let sender = Sender::new(pool.clone(), mailer, clock.clone(), Arc::new(NoExtraChecks));
    tokio::spawn(async move {
        loop {
            if let Err(err) = sender.drain_once(50).await {
                tracing::error!(%err, "sending failed");
            }
            let _ = tokio::time::timeout(TICK, wake.notified()).await;
        }
    });
    tokio::spawn(run_worker(pool.clone(), clock.clone(), choose_model(recorded)?));

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("listening on {bind}"))?;
    tracing::info!(%bind, %base_url, "serving");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("serving")
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
