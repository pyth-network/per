use {
    anyhow::Result,
    clap::Parser,
    std::{
        io::IsTerminal,
        time::Duration,
    },
    tracing_subscriber::filter::LevelFilter,
};

mod config;
mod simulator;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize a Tracing Subscriber
    let fmt_builder = tracing_subscriber::fmt()
        .with_file(false)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_ansi(std::io::stderr().is_terminal());

    // Use the compact formatter if we're in a terminal, otherwise use the JSON formatter.
    if std::io::stderr().is_terminal() {
        tracing::subscriber::set_global_default(fmt_builder.compact().finish())?;
    } else {
        tracing::subscriber::set_global_default(fmt_builder.json().finish())?;
    }

    // Parse the command line arguments with StructOpt, will exit automatically on `--help` or
    // with invalid arguments.
    match config::Options::parse() {
        config::Options::Run(opts) => loop {
            let single_run = simulator::run_simulator(opts.clone()).await;
            if let Err(err) = single_run {
                tracing::error!("Error running simulator: {:?}", err);
            }
            tokio::time::sleep(Duration::from_secs(opts.interval)).await;
        },
        config::Options::CreateSearcher(opts) => {
            simulator::create_searcher(opts).await?;
        }
        config::Options::Deploy(opts) => {
            simulator::deploy_contract(opts).await?;
        }
    };
    Ok(())
}
