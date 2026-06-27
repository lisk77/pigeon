pub mod config;
pub mod history;
mod notification;
pub mod profile;
pub mod serve;

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Serve,
    Profile(ProfileCommand),
    Config(ConfigCommand),
    History(HistoryCommand),
}

#[derive(Args)]
pub struct ProfileCommand {
    #[command(subcommand)]
    pub command: ProfileSubcommand,
}

#[derive(Subcommand)]
pub enum ProfileSubcommand {
    Show,
    List,
    Set { profile: String },
}

#[derive(Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Subcommand)]
pub enum ConfigSubcommand {
    Default,
    Path,
    SetPath { path: PathBuf },
}

#[derive(Args)]
pub struct HistoryCommand {
    #[command(subcommand)]
    pub command: HistorySubcommand,
}

#[derive(Subcommand)]
pub enum HistorySubcommand {
    Show,
    Clear,
    Enable,
    Disable,
}
