//! Pure logic for the `whatif` command: mutate one skattekort line item in the
//! `visningsdata` object and read back the computed tax. The network POST that
//! recomputes tax lives in [`crate::client`]; everything here is testable
//! without it.
//!
//! The portal's `beregn` endpoint takes the current `visningsdata` with a single
//! field changed and returns a recomputed `beregnetSkatt`. To keep the payload
//! structurally valid (it echoes `forekomstId` UUIDs), we never add or remove
//! occurrences: we adjust the **first** occurrence's `beloep` so the field's
//! total reaches the requested value, leaving all ids intact.

use serde_json::Value;

/// How to change a field's amount.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChangeOp {
    /// Set the field's total to this absolute amount.
    Set(f64),
    /// Add this (possibly negative) delta to the field's current total.
    Add(f64),
}

/// Sum of a prognose field's occurrences (`prognoseobjekt[field][].beloep`).
pub fn field_total(visningsdata: &Value, field: &str) -> Option<f64> {
    let arr = visningsdata.get("prognoseobjekt")?.get(field)?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|e| e.get("beloep").and_then(Value::as_f64))
            .sum(),
    )
}

/// Apply `op` to `field` in `visningsdata` (mutating it), returning the new
/// total. Adjusts the first occurrence's `beloep` by the delta so ids are kept.
pub fn apply_change(visningsdata: &mut Value, field: &str, op: ChangeOp) -> Result<f64, String> {
    let current = field_total(visningsdata, field)
        .ok_or_else(|| format!("field '{field}' is not present in your skattekort"))?;
    let new_total = match op {
        ChangeOp::Set(x) => x,
        ChangeOp::Add(d) => current + d,
    };
    let delta = new_total - current;

    let arr = visningsdata
        .get_mut("prognoseobjekt")
        .and_then(|p| p.get_mut(field))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("field '{field}' is not present in your skattekort"))?;
    let first = arr
        .first_mut()
        .ok_or_else(|| format!("field '{field}' has no entries to change"))?;
    let old = first.get("beloep").and_then(Value::as_f64).unwrap_or(0.0);
    first["beloep"] = serde_json::json!(old + delta);
    Ok(new_total)
}

/// Read the total computed tax (`beregnetSkatt.sumSkatt.beloep`) from a response,
/// whether `beregnetSkatt` sits at the root or under a `visningsdata` wrapper.
pub fn sum_skatt(value: &Value) -> Option<f64> {
    let from = |base: &Value| {
        base.get("beregnetSkatt")
            .and_then(|b| b.get("sumSkatt"))
            .and_then(|s| s.get("beloep"))
            .and_then(Value::as_f64)
    };
    from(value).or_else(|| value.get("visningsdata").and_then(from))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "prognoseobjekt": {
                "pensjonsinnbetaling": [
                    { "beloep": 20000.0, "forekomstId": "a" }
                ],
                "paaloepteRenterGenerell": [
                    { "beloep": 100000.0, "forekomstId": "b" },
                    { "beloep": 40000.0, "forekomstId": "c" }
                ]
            },
            "beregnetSkatt": {
                "sumSkatt": { "beloep": 150000.0, "forekomstId": "s" }
            }
        })
    }

    #[test]
    fn field_total_sums_occurrences() {
        assert_eq!(field_total(&sample(), "pensjonsinnbetaling"), Some(20000.0));
        assert_eq!(
            field_total(&sample(), "paaloepteRenterGenerell"),
            Some(140000.0)
        );
        assert_eq!(field_total(&sample(), "nope"), None);
    }

    #[test]
    fn set_replaces_total_via_first_occurrence() {
        let mut v = sample();
        let new_total =
            apply_change(&mut v, "pensjonsinnbetaling", ChangeOp::Set(50000.0)).unwrap();
        assert_eq!(new_total, 50000.0);
        assert_eq!(field_total(&v, "pensjonsinnbetaling"), Some(50000.0));
    }

    #[test]
    fn add_adjusts_first_occurrence_keeping_others_and_ids() {
        let mut v = sample();
        let new_total =
            apply_change(&mut v, "paaloepteRenterGenerell", ChangeOp::Add(10000.0)).unwrap();
        assert_eq!(new_total, 150000.0);
        // Total is now 150000; the second occurrence and all ids are untouched.
        let arr = v["prognoseobjekt"]["paaloepteRenterGenerell"]
            .as_array()
            .unwrap();
        assert_eq!(arr[0]["beloep"].as_f64().unwrap(), 110000.0);
        assert_eq!(arr[0]["forekomstId"], "b");
        assert_eq!(arr[1]["beloep"].as_f64().unwrap(), 40000.0);
        assert_eq!(arr[1]["forekomstId"], "c");
    }

    #[test]
    fn add_can_be_negative() {
        let mut v = sample();
        let new_total =
            apply_change(&mut v, "pensjonsinnbetaling", ChangeOp::Add(-5000.0)).unwrap();
        assert_eq!(new_total, 15000.0);
        assert_eq!(field_total(&v, "pensjonsinnbetaling"), Some(15000.0));
    }

    #[test]
    fn changing_absent_field_errors() {
        let mut v = sample();
        let err = apply_change(&mut v, "doesNotExist", ChangeOp::Set(1.0)).unwrap_err();
        assert!(err.contains("not present"));
    }

    #[test]
    fn sum_skatt_reads_at_root_and_under_visningsdata() {
        assert_eq!(sum_skatt(&sample()), Some(150000.0));
        let wrapped = serde_json::json!({ "visningsdata": sample() });
        assert_eq!(sum_skatt(&wrapped), Some(150000.0));
        assert_eq!(sum_skatt(&serde_json::json!({})), None);
    }
}
