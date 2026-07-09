//! Typed views over the Min skatt JSON responses.
//!
//! Only the fields the table views need are modelled; every command also keeps
//! the raw JSON so `--json` returns the portal's response faithfully (the
//! thin-conduit ethos). Amounts are numbers in skattekort but *strings* in the
//! skattemelding, so the two families use different amount representations.

use std::collections::HashMap;

use serde::Deserialize;

use crate::tekstliste::Tekstliste;

// ---- skattekort (visningsdata/v2) --------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SkattekortResponse {
    pub visningsdata: SkattekortVisningsdata,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkattekortVisningsdata {
    /// Line items keyed by field name; each maps to one or more occurrences
    /// (e.g. several loans under `paaloepteRenterGenerell`).
    #[serde(default)]
    pub prognoseobjekt: HashMap<String, Vec<PrognoseEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrognoseEntry {
    /// The amount; numeric in the skattekort response. Absent on some meta rows.
    #[serde(default)]
    pub beloep: Option<f64>,
}

/// A resolved skattekort line item ready for display.
#[derive(Debug, Clone, PartialEq)]
pub struct LineItem {
    pub field: String,
    pub label: String,
    pub amount: f64,
}

impl SkattekortVisningsdata {
    /// Flatten `prognoseobjekt` into labelled line items, summing occurrences of
    /// the same field. Sorted by label for stable output.
    pub fn line_items(&self, tekster: &Tekstliste) -> Vec<LineItem> {
        let mut items: Vec<LineItem> = self
            .prognoseobjekt
            .iter()
            .map(|(field, entries)| {
                let amount: f64 = entries.iter().filter_map(|e| e.beloep).sum();
                LineItem {
                    field: field.clone(),
                    label: tekster.label_or_field(field),
                    amount,
                }
            })
            .collect();
        items.sort_by_key(|it| it.label.to_lowercase());
        items
    }
}

// ---- skattemelding (hent-gjeldende) ------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkattemeldingResponse {
    #[serde(default)]
    pub dokument_status: Option<String>,
    pub visningsdata: SkattemeldingVisningsdata,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkattemeldingVisningsdata {
    #[serde(default)]
    pub inntektsaar: Option<String>,
    /// Bank accounts: the direct reconciliation target against `sb1`.
    #[serde(default)]
    pub konto: Vec<Konto>,
    /// Employers / salary.
    #[serde(default)]
    pub arbeidsgiver: Vec<Arbeidsgiver>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Konto {
    #[serde(default)]
    pub bankens_navn: Option<String>,
    #[serde(default)]
    pub kontonummer: Option<String>,
    #[serde(default)]
    pub innskudd: Option<Beloep>,
    #[serde(default)]
    pub opptjente_renter: Option<Beloep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arbeidsgiver {
    #[serde(default)]
    pub organisasjonsnavn: Option<String>,
    #[serde(default)]
    pub samlede_ytelser_fra_arbeidsgiver_per_behandlingsart: Vec<Ytelse>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ytelse {
    #[serde(default)]
    pub behandlingsart: Option<String>,
    #[serde(default)]
    pub beloep: Option<Beloep>,
}

/// A skattemelding amount object. Values arrive as *strings* (`"522000"`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Beloep {
    #[serde(default)]
    pub beloep: Option<String>,
}

impl Beloep {
    /// Numeric value, or `0.0` if absent/unparseable.
    pub fn value(&self) -> f64 {
        self.beloep
            .as_deref()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    }
}

impl Konto {
    pub fn innskudd_value(&self) -> f64 {
        self.innskudd.as_ref().map(|b| b.value()).unwrap_or(0.0)
    }
    pub fn renter_value(&self) -> f64 {
        self.opptjente_renter
            .as_ref()
            .map(|b| b.value())
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skattekort_line_items_sum_occurrences_and_get_labels() {
        let resp: SkattekortResponse = serde_json::from_value(serde_json::json!({
            "visningsdata": {
                "prognoseobjekt": {
                    "paaloepteRenterGenerell": [
                        { "beloep": 100000.0, "forekomstId": "a" },
                        { "beloep": 40000.0, "forekomstId": "b" }
                    ],
                    "innskudd": [ { "beloep": 522000.0, "forekomstId": "c" } ]
                }
            }
        }))
        .unwrap();
        let tekster: Tekstliste = serde_json::from_value(serde_json::json!({
            "tekstelementer": {
                "visningsnavn.felt.paaloepteRenterGenerell": { "tekst": "Renter på lån og gjeld" },
                "visningsnavn.felt.innskudd": { "tekst": "Bankinnskudd i 2026" }
            }
        }))
        .unwrap();

        let items = resp.visningsdata.line_items(&tekster);
        // Sorted by label: "Bankinnskudd..." before "Renter...".
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "Bankinnskudd i 2026");
        assert_eq!(items[0].amount, 522000.0);
        assert_eq!(items[1].label, "Renter på lån og gjeld");
        // Two loan occurrences summed.
        assert_eq!(items[1].amount, 140000.0);
    }

    #[test]
    fn line_item_without_label_falls_back_to_field_name() {
        let resp: SkattekortResponse = serde_json::from_value(serde_json::json!({
            "visningsdata": { "prognoseobjekt": { "mysteryField": [ { "beloep": 5.0 } ] } }
        }))
        .unwrap();
        let tekster = Tekstliste {
            tekstelementer: Default::default(),
        };
        let items = resp.visningsdata.line_items(&tekster);
        assert_eq!(items[0].label, "mysteryField");
        assert_eq!(items[0].amount, 5.0);
    }

    #[test]
    fn skattemelding_konto_parses_string_amounts() {
        let resp: SkattemeldingResponse = serde_json::from_value(serde_json::json!({
            "dokumentStatus": "SKATTEMELDING_EGENFASTSATT",
            "visningsdata": {
                "inntektsaar": "2024",
                "konto": [
                    {
                        "bankensNavn": "SPAREBANKEN MØRE",
                        "kontonummer": "39052733573",
                        "innskudd": { "beloep": "522000", "valutakode": "NOK" },
                        "opptjenteRenter": { "beloep": "7217" }
                    }
                ]
            }
        }))
        .unwrap();
        assert_eq!(
            resp.dokument_status.as_deref(),
            Some("SKATTEMELDING_EGENFASTSATT")
        );
        let v = resp.visningsdata;
        assert_eq!(v.inntektsaar.as_deref(), Some("2024"));
        assert_eq!(v.konto.len(), 1);
        let k = &v.konto[0];
        assert_eq!(k.bankens_navn.as_deref(), Some("SPAREBANKEN MØRE"));
        assert_eq!(k.innskudd_value(), 522000.0);
        assert_eq!(k.renter_value(), 7217.0);
    }

    #[test]
    fn beloep_absent_or_bad_is_zero() {
        assert_eq!(Beloep { beloep: None }.value(), 0.0);
        assert_eq!(
            Beloep {
                beloep: Some("not a number".into())
            }
            .value(),
            0.0
        );
        assert_eq!(
            Beloep {
                beloep: Some("  1234  ".into())
            }
            .value(),
            1234.0
        );
    }
}
