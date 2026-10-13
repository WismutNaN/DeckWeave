// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_automation::CatalogSchema;
use serde_json::json;

#[test]
fn numeric_schema_limits_remain_identical_through_catalog_storage_and_projection()
-> Result<(), Box<dyn std::error::Error>> {
    for maximum in [9_007_199_254_740_991.0, f64::MAX, 0.1, 1.0e-308] {
        let schema: CatalogSchema = serde_json::from_value(json!({
            "schema":{"id":"org.deckweave.roundtrip","version":{"major":1,"minor":0}},
            "valueKind":{"structured":{"type":"record","fields":{"value":{"schema":{"type":"number","minimum":0,"maximum":maximum}}}}},
            "unit":null,"range":null
        }))?;
        let mut restored = schema.clone();
        for _ in 0..3 {
            restored = serde_json::from_slice(&serde_json::to_vec(&restored)?)?;
            assert_eq!(schema, restored, "numeric bound {maximum} drifted");
        }
    }
    Ok(())
}
