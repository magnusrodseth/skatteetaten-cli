//! Read commands: skattekort, skattemelding, oppgjør.

use anyhow::{anyhow, Context};

use crate::cli::{WhatifArgs, YearArgs};
use crate::format::{self, OutputMode};
use crate::whatif::{self, ChangeOp};

pub fn skattekort(mode: OutputMode, mask: bool) -> anyhow::Result<()> {
    let client = crate::commands::authed_client()?;

    if mode == OutputMode::Json {
        let raw = client.skattekort_raw().context("fetching skattekort")?;
        return format::print_json(&raw);
    }

    let year = client.active_year().unwrap_or(None);
    let data = client.skattekort().context("fetching skattekort")?;
    let tekster = client
        .tekstliste()
        .context("fetching skattekort label dictionary")?;
    let items = data.visningsdata.line_items(&tekster);
    format::skattekort_table(&items, year, mask);
    Ok(())
}

pub fn skattemelding(args: YearArgs, mode: OutputMode, mask: bool) -> anyhow::Result<()> {
    let client = crate::commands::authed_client()?;
    let year = match args.year {
        Some(y) => y,
        None => default_skattemelding_year(&client),
    };

    if mode == OutputMode::Json {
        let raw = client
            .skattemelding_raw(year)
            .with_context(|| format!("fetching skattemelding {year}"))?;
        return format::print_json(&raw);
    }

    let data = client
        .skattemelding(year)
        .with_context(|| format!("fetching skattemelding {year}"))?;
    if let Some(status) = &data.dokument_status {
        println!("Status: {status}");
    }
    format::skattemelding_table(&data.visningsdata, mask);
    Ok(())
}

pub fn oppgjor(mode: OutputMode) -> anyhow::Result<()> {
    let client = crate::commands::authed_client()?;
    let raw = client.oppgjor_raw().context("fetching skatteoppgjør")?;
    if mode == OutputMode::Json {
        return format::print_json(&raw);
    }
    // The landing is a deep, nested `blokker` tree; rather than guess a fragile
    // table shape, print the raw JSON and point at --json. This keeps the tool a
    // faithful, thin conduit for the one endpoint whose shape is least stable.
    format::print_json(&raw)
}

pub fn whatif(args: WhatifArgs, mode: OutputMode, mask: bool) -> anyhow::Result<()> {
    let op = match (args.set, args.add) {
        (Some(x), None) => ChangeOp::Set(x),
        (None, Some(d)) => ChangeOp::Add(d),
        _ => return Err(anyhow!("provide exactly one of --set or --add")),
    };

    let client = crate::commands::authed_client()?;

    // Fetch the current skattekort, note the current tax, mutate one field, and
    // ask the portal to recompute. Nothing is persisted.
    let mut body = client.skattekort_raw().context("fetching skattekort")?;
    let visningsdata = body
        .get_mut("visningsdata")
        .ok_or_else(|| anyhow!("unexpected skattekort response (no visningsdata)"))?;

    let before_tax = whatif::sum_skatt(visningsdata);
    let old_total = whatif::field_total(visningsdata, &args.field);
    let new_total = whatif::apply_change(visningsdata, &args.field, op).map_err(|e| anyhow!(e))?;

    let recomputed = client.beregn(&body).context("recomputing tax (beregn)")?;
    let after_tax = whatif::sum_skatt(&recomputed);

    if mode == OutputMode::Json {
        return format::print_json(&serde_json::json!({
            "field": args.field,
            "fieldBefore": old_total,
            "fieldAfter": new_total,
            "taxBefore": before_tax,
            "taxAfter": after_tax,
            "taxDelta": match (before_tax, after_tax) {
                (Some(b), Some(a)) => Some(a - b),
                _ => None,
            },
        }));
    }

    format::whatif_table(
        &args.field,
        old_total,
        new_total,
        before_tax,
        after_tax,
        mask,
    );
    Ok(())
}

/// Best-effort default income year for the skattemelding: the year *before* the
/// active skattekort year (the most recent return that has been filed). Falls
/// back to last calendar year if the portal config is unavailable.
fn default_skattemelding_year(client: &crate::client::ApiClient) -> i32 {
    if let Ok(Some(active)) = client.active_year() {
        return active - 1;
    }
    let this_year: i32 = chrono::Local::now()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2025);
    this_year - 1
}
