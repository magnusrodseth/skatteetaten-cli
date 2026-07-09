//! Output rendering: pretty tables (default) and JSON.
//!
//! Tables use Norwegian amount formatting (`kr 1 234,56`). `--json` emits the
//! portal's response (or a typed view) unchanged for scripting and agents.

use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};
use serde::Serialize;

use crate::models::{LineItem, SkattemeldingVisningsdata};
use crate::util::{kr, MASKED};

/// Global output mode, set from the top-level `--json` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Table,
    Json,
}

/// Print any serialisable value as pretty JSON.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn base_table() -> Table {
    let mut t = Table::new();
    t.load_preset(comfy_table::presets::UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    t
}

/// Render skattekort line items as a labelled amount table.
pub fn skattekort_table(items: &[LineItem], year: Option<i32>, mask: bool) {
    let mut table = base_table();
    table.set_header(vec!["Post", "Beløp", "Felt"]);
    for it in items {
        table.add_row(vec![
            Cell::new(&it.label),
            Cell::new(kr(it.amount, mask)).set_alignment(CellAlignment::Right),
            Cell::new(&it.field),
        ]);
    }
    if let Some(y) = year {
        println!("Skattekort {y}");
    }
    println!("{table}");
}

/// Render the skattemelding: bank accounts (with interest) and salary.
pub fn skattemelding_table(v: &SkattemeldingVisningsdata, mask: bool) {
    if let Some(year) = &v.inntektsaar {
        println!("Skattemelding {year}");
    }

    if !v.konto.is_empty() {
        let mut table = base_table();
        table.set_header(vec!["Bank", "Kontonummer", "Innskudd", "Renter"]);
        let mut sum_innskudd = 0.0;
        let mut sum_renter = 0.0;
        for k in &v.konto {
            let number = if mask {
                MASKED.to_string()
            } else {
                k.kontonummer.clone().unwrap_or_default()
            };
            sum_innskudd += k.innskudd_value();
            sum_renter += k.renter_value();
            table.add_row(vec![
                Cell::new(k.bankens_navn.clone().unwrap_or_default()),
                Cell::new(number),
                Cell::new(kr(k.innskudd_value(), mask)).set_alignment(CellAlignment::Right),
                Cell::new(kr(k.renter_value(), mask)).set_alignment(CellAlignment::Right),
            ]);
        }
        println!("\nBankinnskudd og renter:");
        println!("{table}");
        println!(
            "Sum innskudd: {}   Sum renter: {}",
            kr(sum_innskudd, mask),
            kr(sum_renter, mask)
        );
    }

    if !v.arbeidsgiver.is_empty() {
        let mut table = base_table();
        table.set_header(vec!["Arbeidsgiver", "Ytelse", "Beløp"]);
        for a in &v.arbeidsgiver {
            let name = a.organisasjonsnavn.clone().unwrap_or_default();
            for y in &a.samlede_ytelser_fra_arbeidsgiver_per_behandlingsart {
                table.add_row(vec![
                    Cell::new(&name),
                    Cell::new(y.behandlingsart.clone().unwrap_or_default()),
                    Cell::new(kr(
                        y.beloep.as_ref().map(|b| b.value()).unwrap_or(0.0),
                        mask,
                    ))
                    .set_alignment(CellAlignment::Right),
                ]);
            }
        }
        println!("\nLønn og ytelser:");
        println!("{table}");
    }
}

/// Render a what-if result: the changed field, and tax before vs after.
pub fn whatif_table(
    field: &str,
    field_before: Option<f64>,
    field_after: f64,
    tax_before: Option<f64>,
    tax_after: Option<f64>,
    mask: bool,
) {
    let mut table = base_table();
    table.set_header(vec!["", "Før", "Etter", "Endring"]);
    let row = |label: &str, before: Option<f64>, after: Option<f64>| {
        let delta = match (before, after) {
            (Some(b), Some(a)) => kr(a - b, mask),
            _ => "-".to_string(),
        };
        vec![
            Cell::new(label),
            Cell::new(before.map(|b| kr(b, mask)).unwrap_or_else(|| "-".into()))
                .set_alignment(CellAlignment::Right),
            Cell::new(after.map(|a| kr(a, mask)).unwrap_or_else(|| "-".into()))
                .set_alignment(CellAlignment::Right),
            Cell::new(delta).set_alignment(CellAlignment::Right),
        ]
    };
    table.add_row(row(field, field_before, Some(field_after)));
    table.add_row(row("Sum skatt", tax_before, tax_after));
    println!("What-if (nothing was saved):");
    println!("{table}");
}
