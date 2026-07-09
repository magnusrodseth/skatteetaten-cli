//! Command-line surface (clap derive).

use clap::{Args, Parser, Subcommand};

/// Unofficial, read-only CLI for your OWN Norwegian tax data (Min skatt).
///
/// Log in once with BankID (`skt login`), then read your skattekort,
/// skattemelding and skatteoppgjør. Every command supports `--json`. This tool
/// is not affiliated with Skatteetaten and never submits or edits anything.
#[derive(Debug, Parser)]
#[command(name = "skt", version, about, long_about = None)]
pub struct Cli {
    /// Output machine-readable JSON instead of tables.
    #[arg(long, global = true)]
    pub json: bool,

    /// Mask sensitive values (amounts, account numbers) in table output, for
    /// sharing screenshots. Labels and dates stay visible. No effect on --json.
    #[arg(long, global = true)]
    pub mask: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Log in with BankID in a browser and capture the Min skatt session.
    Login,

    /// Remove the stored session.
    Logout,

    /// Show authentication status and which storage backend is active.
    Status,

    /// Verify the session and print the logged-in person's name.
    Whoami,

    /// Show your skattekort (tax card) line items for the active year.
    Skattekort,

    /// Show your skattemelding (tax return) for a year.
    Skattemelding(YearArgs),

    /// Show your skatteoppgjør / Min skatt landing (restskatt, konto, KID).
    Oppgjor,

    /// What-if: recompute your tax with one skattekort field changed. Pure
    /// calculation against the portal — persists nothing.
    Whatif(WhatifArgs),
}

#[derive(Debug, Args)]
pub struct WhatifArgs {
    /// Skattekort field to change, e.g. `pensjonsinnbetaling`,
    /// `paaloepteRenterGenerell`, `innskudd`. Run `skt skattekort` to see the
    /// available field names (the rightmost "Felt" column).
    #[arg(long)]
    pub field: String,

    /// Set the field's total to this absolute amount (NOK).
    #[arg(long, conflicts_with = "add", required_unless_present = "add")]
    pub set: Option<f64>,

    /// Add this (possibly negative) delta to the field's current amount (NOK).
    #[arg(long, allow_hyphen_values = true)]
    pub add: Option<f64>,
}

#[derive(Debug, Args)]
pub struct YearArgs {
    /// Income year (e.g. 2024). Defaults to the most recent completed return.
    #[arg(long)]
    pub year: Option<i32>,
}
