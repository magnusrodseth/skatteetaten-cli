//! The skattekort label dictionary (`/api/skattekort/tekstliste?locale=nb`).
//!
//! Line-item values in `visningsdata/v2` are keyed by self-documenting field
//! names (`paaloepteRenterGenerell`, `innskudd`, …) but carry no human labels.
//! This dictionary supplies them: for a field `X`, its display label is the
//! `tekst` of the element keyed `visningsnavn.felt.X`.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Tekstliste {
    #[serde(default)]
    pub tekstelementer: HashMap<String, Tekstelement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tekstelement {
    #[serde(default)]
    pub tekst: String,
}

impl Tekstliste {
    /// Human display label for a prognose field, e.g. `paaloepteRenterGenerell`
    /// -> "Renter på lån og gjeld". `None` if the dictionary has no entry.
    pub fn label_for(&self, field: &str) -> Option<String> {
        let key = format!("visningsnavn.felt.{field}");
        self.tekstelementer
            .get(&key)
            .map(|e| e.tekst.clone())
            .filter(|t| !t.is_empty())
    }

    /// Label if known, otherwise the raw field name (so output never goes blank).
    pub fn label_or_field(&self, field: &str) -> String {
        self.label_for(field).unwrap_or_else(|| field.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Tekstliste {
        serde_json::from_value(serde_json::json!({
            "locale": "nb",
            "tekstelementer": {
                "visningsnavn.felt.paaloepteRenterGenerell": { "tekst": "Renter på lån og gjeld" },
                "visningsnavn.felt.innskudd": { "tekst": "Bankinnskudd i 2026" },
                "hjelpetekst.felt.innskudd": { "tekst": "help text, not a label" },
                "visningsnavn.felt.tomtFelt": { "tekst": "" }
            }
        }))
        .unwrap()
    }

    #[test]
    fn resolves_display_label_for_known_field() {
        assert_eq!(
            sample().label_for("paaloepteRenterGenerell").as_deref(),
            Some("Renter på lån og gjeld")
        );
        assert_eq!(
            sample().label_for("innskudd").as_deref(),
            Some("Bankinnskudd i 2026")
        );
    }

    #[test]
    fn unknown_field_has_no_label() {
        assert_eq!(sample().label_for("nonexistentField"), None);
    }

    #[test]
    fn empty_label_is_treated_as_absent() {
        assert_eq!(sample().label_for("tomtFelt"), None);
    }

    #[test]
    fn label_or_field_falls_back_to_field_name() {
        assert_eq!(
            sample().label_or_field("paaloepteRenterGenerell"),
            "Renter på lån og gjeld"
        );
        assert_eq!(sample().label_or_field("mysteryField"), "mysteryField");
    }
}
