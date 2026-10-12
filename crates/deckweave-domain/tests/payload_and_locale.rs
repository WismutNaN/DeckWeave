// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    localization::{LocaleContext, TranslationCatalog},
    schema::PayloadSchema,
};
use serde_json::json;

#[test]
fn structured_payload_rejects_missing_nested_unknown_and_out_of_range_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let schema: PayloadSchema = serde_json::from_value(json!({"type":"record","fields":{
        "title":{"schema":{"type":"string","maximumBytes":12}},
        "items":{"schema":{"type":"array","maximumItems":2,"items":{"type":"number","minimum":0,"maximum":1}}},
        "description":{"optional":true,"schema":{"type":"nullable","inner":{"type":"string","maximumBytes":30}}}
    }}))?;
    schema.validate_definition()?;
    assert!(schema.accepts(&json!({"title":"Задача","items":[0,0.5],"description":null})));
    for bad in [
        json!({"title":"a"}),
        json!({"title":"a","items":[1.1]}),
        json!({"title":"a","items":[0,0,0]}),
        json!({"title":"a","items":[],"typo":true}),
        json!({"title":4,"items":[]}),
    ] {
        assert!(!schema.accepts(&bad), "accepted {bad}");
    }
    assert!(
        serde_json::from_value::<PayloadSchema>(
            json!({"type":"string","maximumBytes":30,"requiredPattern":"x"})
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn locale_fallback_is_per_message_and_keeps_explicit_default()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog: TranslationCatalog =
        serde_json::from_value(json!({"defaultLocale":"en","messages":{
            "en":{"title":"Title","missing":"English fallback"},
            "ru":{"title":"Заголовок"}, "ru-ru":{"regional":"Локальный"}
        }}))?;
    catalog.validate()?;
    assert_eq!(catalog.resolve("ru-RU", "title", "fallback"), "Заголовок");
    assert_eq!(
        catalog.resolve("ru-RU", "missing", "fallback"),
        "English fallback"
    );
    assert_eq!(catalog.resolve("de-DE", "absent", "fallback"), "fallback");
    assert!(
        LocaleContext {
            requested_locale: "../../ru".into(),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    Ok(())
}
