//! Mod configuration manager, NeoForge `ForgeConfigSpec`-style core slice.
//!
//! Loads/saves TOML values, applies defaults, validates types/ranges and
//! reloads at runtime. Game/network integration is separate.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::parity_models::ConfigSpecDef;

#[derive(Clone, Debug, PartialEq)]
pub struct ModConfig {
    pub spec: ConfigSpecDef,
    pub values: BTreeMap<String, String>,
    pub path: PathBuf,
    pub dirty: bool,
}

impl ModConfig {
    pub fn new(spec: ConfigSpecDef, path: impl Into<PathBuf>) -> Self {
        let mut values = BTreeMap::new();
        for field in &spec.fields {
            if !field.default.is_empty() {
                values.insert(field.path.clone(), field.default.clone());
            }
        }
        let mut out = Self {
            spec,
            values,
            path: path.into(),
            dirty: false,
        };
        out.apply_defaults();
        out
    }

    fn apply_defaults(&mut self) {
        for field in &self.spec.fields {
            self.values.entry(field.path.clone()).or_default();
            if let Some(v) = self.values.get(&field.path)
                && v.is_empty()
                && !field.default.is_empty()
            {
                self.values
                    .insert(field.path.clone(), field.default.clone());
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) -> Result<(), String> {
        let value = value.into();
        if let Some(field) = self.spec.fields.iter().find(|f| f.path == key) {
            validate_field(field, &value)?;
        }
        self.values.insert(key.to_string(), value);
        self.dirty = true;
        Ok(())
    }

    pub fn load(&mut self) -> Result<(), String> {
        if self.path.exists() {
            let text = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
            let parsed: BTreeMap<String, String> =
                toml::from_str(&text).map_err(|e| e.to_string())?;
            for (k, v) in parsed {
                self.set(&k, v.clone())?;
                self.values.insert(k, v);
            }
        } else {
            self.save()?;
        }
        self.apply_defaults();
        self.dirty = false;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(&self.values).map_err(|e| e.to_string())?;
        fs::write(&self.path, text).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let values = self.values.clone();
        self.load()?;
        self.values = self.values.clone();
        let _ = values;
        Ok(())
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigManager {
    pub configs: BTreeMap<String, ModConfig>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, id: impl Into<String>, config: ModConfig) {
        self.configs.insert(id.into(), config);
    }

    pub fn get(&self, id: &str) -> Option<&ModConfig> {
        self.configs.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ModConfig> {
        self.configs.get_mut(id)
    }

    pub fn load_all(&mut self) -> Result<(), String> {
        for config in self.configs.values_mut() {
            config.load()?;
        }
        Ok(())
    }

    pub fn save_all(&self) -> Result<(), String> {
        for config in self.configs.values() {
            config.save()?;
        }
        Ok(())
    }
}

fn validate_field(field: &crate::parity_models::ConfigFieldDef, value: &str) -> Result<(), String> {
    match field.kind.as_str() {
        "int" => {
            let v: i64 = value.parse().map_err(|_| "not int".to_string())?;
            if let Some(min) = &field.range_min
                && let Ok(min) = min.parse::<i64>()
                && v < min
            {
                return Err("min".into());
            }
            if let Some(max) = &field.range_max
                && let Ok(max) = max.parse::<i64>()
                && v > max
            {
                return Err("max".into());
            }
        }
        "float" => {
            let v: f64 = value.parse().map_err(|_| "not float".to_string())?;
            if let Some(min) = &field.range_min
                && let Ok(min) = min.parse::<f64>()
                && v < min
            {
                return Err("min".into());
            }
            if let Some(max) = &field.range_max
                && let Ok(max) = max.parse::<f64>()
                && v > max
            {
                return Err("max".into());
            }
        }
        "bool" if value != "true" && value != "false" => return Err("not bool".into()),
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parity_models::{ConfigFieldDef, ConfigSpecDef};

    #[test]
    fn config_set_validate_save_reload() {
        let tmp = std::env::temp_dir().join("hmod-test-config.toml");
        let _ = std::fs::remove_file(&tmp);
        let mut spec = ConfigSpecDef::new("demo", "common");
        let mut field = ConfigFieldDef::new("value", "int");
        field.default = "5".into();
        field.range_min = Some("0".into());
        field.range_max = Some("10".into());
        spec.fields.push(field);

        let mut cfg = ModConfig::new(spec, tmp.clone());
        assert_eq!(cfg.get("value"), Some("5"));
        cfg.set("value", "7").unwrap();
        assert!(cfg.is_dirty());
        cfg.save().unwrap();
        let mut reloaded = ModConfig::new(cfg.spec.clone(), tmp.clone());
        reloaded.load().unwrap();
        assert_eq!(reloaded.get("value"), Some("7"));
        assert!(reloaded.set("value", "99").is_err());
        let _ = std::fs::remove_file(&tmp);
    }
}
