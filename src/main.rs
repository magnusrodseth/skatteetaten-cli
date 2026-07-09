//! `skt`: an unofficial, read-only CLI for a private person's own Norwegian tax
//! data from Skatteetaten's Min skatt portal.
//!
//! There is no official personal API here; `skt` replays an authenticated
//! browser session against undocumented internal JSON endpoints, and only ever
//! reads. See [`crate::terms`] for the constraints this shapes.

mod auth;
mod cli;
mod client;
mod commands;
mod error;
mod format;
mod models;
mod secrets;
mod session;
mod tekstliste;
mod terms;
mod util;
mod whatif;

use clap::Parser;

use crate::cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = commands::run(cli) {
        eprintln!("error: {err}");
        for cause in err.chain().skip(1) {
            eprintln!("  caused by: {cause}");
        }
        std::process::exit(1);
    }
}
