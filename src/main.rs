use clap::Parser;
use pigeon::cli::{
    Cli, Command, ConfigSubcommand, HistorySubcommand, ProfileSubcommand, config::*, history::*,
    profile::*, serve::serve,
};
use tracing_subscriber::{EnvFilter, fmt};

fn main() {
    pigeon::memory::tune_allocator_for_low_memory();

    if std::env::var_os(pigeon::sound::HELPER_ENV).is_some() {
        if let Err(error) = pigeon::sound::run_helper(std::env::args_os().skip(1)) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return;
    }

    init_logging();

    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_env("PIGEON_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Serve => serve(),
        Command::Profile(cmd) => match cmd.command {
            ProfileSubcommand::Show => profile_show(),
            ProfileSubcommand::List => profile_list(),
            ProfileSubcommand::Set { profile } => profile_set(profile),
        },
        Command::Config(cmd) => match cmd.command {
            ConfigSubcommand::Default => config_default(),
            ConfigSubcommand::Path => config_path(),
            ConfigSubcommand::SetPath { path } => config_set_path(path),
        },
        Command::History(cmd) => match cmd.command {
            HistorySubcommand::Show => history_show(),
            HistorySubcommand::Clear => history_clear(),
            HistorySubcommand::Enable => history_enable(),
            HistorySubcommand::Disable => history_disable(),
        },
    }
}
