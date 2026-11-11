use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::rule::{Rule, Tap};

pub type KeyTable = [(&'static str, u16)];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemapEntry {
    pub from: String,
    pub to: String,
    /// 変換後のキーを後段のルール（`tap` など）で変換しない。
    #[serde(rename = "final", default)]
    pub is_final: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TapEntry {
    pub key: String,
    #[serde(default)]
    pub hold: Option<String>,
    pub alone: String,
    #[serde(default = "default_tap_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_tap_timeout_ms() -> u64 {
    1000
}

pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn key(table: &KeyTable, name: &str) -> Result<u16, String> {
    table
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, code)| *code)
        .ok_or_else(|| format!("unknown key name: {name}"))
}

pub fn remap_pairs(entries: &[RemapEntry], table: &KeyTable) -> Result<Vec<(u16, u16)>, String> {
    entries
        .iter()
        .map(|e| Ok((key(table, &e.from)?, key(table, &e.to)?)))
        .collect()
}

#[cfg(any(windows, test))]
pub fn final_keys(entries: &[RemapEntry], table: &KeyTable) -> Result<Vec<u16>, String> {
    entries
        .iter()
        .filter(|e| e.is_final)
        .map(|e| key(table, &e.from))
        .collect()
}

pub fn tap_rules(entries: &[TapEntry], table: &KeyTable) -> Result<Vec<Box<dyn Rule>>, String> {
    entries
        .iter()
        .map(|e| {
            let rule = Tap::new(
                key(table, &e.key)?,
                e.hold.as_deref().map(|h| key(table, h)).transpose()?,
                key(table, &e.alone)?,
                Duration::from_millis(e.timeout_ms),
            );
            Ok(Box::new(rule) as Box<dyn Rule>)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &KeyTable = &[("a", 1), ("b", 2)];

    #[test]
    fn resolves_names() {
        let entries = [RemapEntry {
            from: "a".into(),
            to: "b".into(),
            is_final: false,
        }];
        assert_eq!(remap_pairs(&entries, TABLE).unwrap(), [(1, 2)]);
    }

    #[test]
    fn rejects_unknown_name() {
        let entries = [RemapEntry {
            from: "a".into(),
            to: "c".into(),
            is_final: false,
        }];
        assert_eq!(
            remap_pairs(&entries, TABLE).unwrap_err(),
            "unknown key name: c"
        );
    }

    #[test]
    fn tap_timeout_defaults() {
        #[derive(Deserialize)]
        struct Config {
            tap: Vec<TapEntry>,
        }
        let config: Config = toml::from_str("[[tap]]\nkey = \"a\"\nalone = \"b\"\n").unwrap();
        assert_eq!(config.tap[0].timeout_ms, 1000);
    }

    #[test]
    fn collects_final_keys() {
        #[derive(Deserialize)]
        struct Config {
            remap: Vec<RemapEntry>,
        }
        let config: Config = toml::from_str(
            "[[remap]]\nfrom = \"a\"\nto = \"b\"\nfinal = true\n\n[[remap]]\nfrom = \"b\"\nto = \"a\"\n",
        )
        .unwrap();
        assert_eq!(final_keys(&config.remap, TABLE).unwrap(), [1]);
    }
}
