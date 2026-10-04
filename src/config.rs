use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub drawer: bool,
    pub theme: Theme,
    pub favorites: Vec<String>,
    pub order: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("mango-layout-tray/config.toml")
    }
    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let config: Self =
            toml::from_str(&std::fs::read_to_string(&path)?).context("Invalid configuration")?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        for name in self.favorites.iter().chain(&self.order) {
            if crate::layout::by_name(name).is_none() {
                bail!("Unknown layout in configuration: {name}");
            }
        }
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = Self::path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, toml::to_string_pretty(self)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
    pub fn layouts(&self) -> Vec<crate::layout::Layout> {
        let mut layouts = Vec::new();
        for name in self.favorites.iter().chain(&self.order) {
            if let Some(layout) = crate::layout::by_name(name)
                && !layouts.contains(&layout)
            {
                layouts.push(layout);
            }
        }
        for layout in crate::layout::LAYOUTS {
            if !layouts.contains(&layout) {
                layouts.push(layout);
            }
        }
        layouts
    }
}

pub fn autostart_path() -> PathBuf {
    config_dir().join("autostart/mango-layout-tray.desktop")
}
pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = autostart_path();
    if enabled {
        let binary = std::env::current_exe()?;
        let executable = binary
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%");
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(
            path,
            format!(
                "[Desktop Entry]\nType=Application\nName=Mango Layout Tray\nExec=\"{executable}\"\nIcon=mango-layout-tray\nTerminal=false\n"
            ),
        )?;
    } else if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_rejects_typos_and_keeps_every_layout() {
        assert!(toml::from_str::<Config>("drawre = true").is_err());
        let c = Config {
            favorites: vec!["dwindle".into(), "dwindle".into()],
            order: vec!["tile".into()],
            ..Default::default()
        };
        assert_eq!(c.layouts().len(), 14);
        assert_eq!(c.layouts()[0].name, "dwindle");
        assert_eq!(c.layouts()[1].name, "tile");
        assert!(
            Config {
                order: vec!["typo".into()],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
