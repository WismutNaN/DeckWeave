// SPDX-License-Identifier: GPL-3.0-or-later
//! Локаль представления не меняет идентификаторы, schemas или данные автоматизации.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocaleContext {
    pub revision: u64,
    pub requested_locale: String,
    pub time_zone: String,
}

impl Default for LocaleContext {
    fn default() -> Self {
        Self {
            revision: 1,
            requested_locale: "en".into(),
            time_zone: "UTC".into(),
        }
    }
}

impl LocaleContext {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.revision == 0 || normalize_locale(&self.requested_locale).is_none() {
            return Err("invalid locale context");
        }
        // Каталог IANA и OS mapping принадлежит platform adapter; здесь проверяется wire bound.
        if self.time_zone.is_empty()
            || self.time_zone.len() > 128
            || !self
                .time_zone
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        {
            return Err("invalid time zone identifier");
        }
        Ok(())
    }
}

pub fn normalize_locale(value: &str) -> Option<String> {
    let parts: Vec<_> = value.split('-').collect();
    if value.len() > 63
        || parts.is_empty()
        || !(2..=8).contains(&parts[0].len())
        || !parts[0].bytes().all(|b| b.is_ascii_alphabetic())
        || parts
            .iter()
            .skip(1)
            .any(|p| p.is_empty() || p.len() > 8 || !p.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

/// Детерминированный lookup: requested → родители → package default.
/// При отсутствии key используется fallback самого сообщения, не случайный язык.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranslationCatalog {
    pub default_locale: String,
    pub messages: BTreeMap<String, BTreeMap<String, String>>,
}

impl TranslationCatalog {
    pub fn validate(&self) -> Result<(), &'static str> {
        let default = normalize_locale(&self.default_locale).ok_or("invalid default locale")?;
        if self.messages.len() > 64 || !self.messages.contains_key(&default) {
            return Err("missing default locale");
        }
        for (locale, messages) in &self.messages {
            if normalize_locale(locale).as_ref() != Some(locale)
                || messages.len() > 8192
                || messages
                    .iter()
                    .any(|(k, v)| k.is_empty() || k.len() > 128 || v.len() > 8192)
            {
                return Err("invalid translation catalog");
            }
        }
        Ok(())
    }

    pub fn resolve<'a>(&'a self, requested: &str, key: &str, fallback: &'a str) -> &'a str {
        let mut locale = normalize_locale(requested).unwrap_or_default();
        loop {
            if let Some(text) = self.messages.get(&locale).and_then(|m| m.get(key)) {
                return text;
            }
            let Some(index) = locale.rfind('-') else {
                break;
            };
            locale.truncate(index);
            if locale
                .rsplit('-')
                .next()
                .is_some_and(|part| part.len() == 1)
                && let Some(index) = locale.rfind('-')
            {
                locale.truncate(index);
            }
        }
        normalize_locale(&self.default_locale)
            .and_then(|locale| self.messages.get(&locale))
            .and_then(|m| m.get(key))
            .map_or(fallback, String::as_str)
    }
}
