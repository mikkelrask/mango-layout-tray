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

const STARTUP_BEGIN: &str = "# BEGIN mango-layout-tray autostart";
const STARTUP_END: &str = "# END mango-layout-tray autostart";

pub fn mango_config_path() -> Result<PathBuf> {
    let default = PathBuf::from(std::env::var_os("HOME").context("HOME is unset")?)
        .join(".config/mango/config.conf");
    // Mango's socket identifies the compositor owning this session.
    if let Some(signature) = std::env::var_os("MANGO_INSTANCE_SIGNATURE") {
        let socket = PathBuf::from(signature);
        if let Some(pid) = socket
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("mango-"))
            .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        {
            let process = PathBuf::from(format!("/proc/{pid}"));
            if std::fs::read_to_string(process.join("comm"))
                .unwrap_or_default()
                .trim()
                == "mango"
            {
                let args = std::fs::read(process.join("cmdline"))?;
                let args: Vec<_> = args.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
                if let Some(pair) = args.windows(2).find(|pair| pair[0] == b"-c") {
                    use std::os::unix::ffi::OsStrExt;
                    let path = PathBuf::from(std::ffi::OsStr::from_bytes(pair[1]));
                    return Ok(if path.is_absolute() {
                        path
                    } else {
                        std::fs::read_link(process.join("cwd"))?.join(path)
                    });
                }
            }
        }
    }
    Ok(default)
}

pub fn autostart_enabled() -> Result<bool> {
    let path = mango_config_path()?;
    let text = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    Ok(text.lines().any(|line| line == STARTUP_BEGIN))
}

fn startup_contents(text: &str, executable: Option<&str>) -> Result<String> {
    let mut result = String::new();
    let mut inside = false;
    for line in text.split_inclusive('\n') {
        let marker = line.trim_end_matches(['\r', '\n']);
        if marker == STARTUP_BEGIN {
            if inside {
                bail!("Duplicate autostart marker in Mango configuration");
            }
            inside = true;
        } else if marker == STARTUP_END {
            if !inside {
                bail!("Unmatched autostart marker in Mango configuration");
            }
            inside = false;
        } else if !inside {
            result.push_str(line);
        }
    }
    if inside {
        bail!("Unclosed autostart block in Mango configuration");
    }
    if let Some(executable) = executable {
        if executable.contains(['\n', '\r', '#']) {
            bail!("Executable path contains characters unsupported by Mango configuration");
        }
        let quoted = executable.replace('\'', "'\"'\"'");
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!(
            "{STARTUP_BEGIN}\nexec-once='{quoted}'\n{STARTUP_END}\n"
        ));
    }
    Ok(result)
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = mango_config_path()?;
    let path = if path.exists() {
        std::fs::canonicalize(path)?
    } else {
        path
    };
    let original = if path.exists() {
        std::fs::read_to_string(&path)?
    } else if enabled {
        std::fs::read_to_string("/etc/mango/config.conf").context(
            "No Mango configuration found. Create your Mango configuration before enabling startup",
        )?
    } else {
        String::new()
    };
    let binary = std::env::current_exe()?;
    let binary = binary.to_str().context("Executable path is not UTF-8")?;
    let updated = startup_contents(&original, enabled.then_some(binary))?;
    if updated != original {
        std::fs::create_dir_all(
            path.parent()
                .context("Mango configuration has no parent directory")?,
        )?;
        if path.exists() {
            let backup = path.with_extension("conf.mango-layout-tray.bak");
            if !backup.exists() {
                std::fs::copy(&path, backup)?;
            }
        }
        let temporary = path.with_extension("conf.mango-layout-tray.tmp");
        std::fs::write(&temporary, updated)?;
        if path.exists() {
            std::fs::set_permissions(&temporary, std::fs::metadata(&path)?.permissions())?;
        }
        std::fs::rename(temporary, path)?;
    }
    let legacy = config_dir().join("autostart/mango-layout-tray.desktop");
    if legacy.exists() {
        std::fs::remove_file(legacy)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_preserves_user_commands_and_is_idempotent() {
        let original = "# My startup\r\nexec-once=waybar\r\nexec-once=mango-layout-tray\r\n";
        let enabled = startup_contents(original, Some("/tmp/my app's tray")).unwrap();
        assert!(enabled.starts_with(original));
        assert!(enabled.contains("exec-once='/tmp/my app'\"'\"'s tray'"));
        assert_eq!(
            startup_contents(&enabled, Some("/tmp/my app's tray")).unwrap(),
            enabled
        );
        assert_eq!(startup_contents(&enabled, None).unwrap(), original);
        assert_eq!(startup_contents(original, None).unwrap(), original);
    }

    #[test]
    fn startup_rejects_broken_blocks_and_unsafe_paths() {
        assert!(startup_contents(STARTUP_BEGIN, None).is_err());
        assert!(startup_contents(STARTUP_END, None).is_err());
        assert!(startup_contents("", Some("/tmp/tray\nexec=other")).is_err());
        assert!(startup_contents("", Some("/tmp/tray#name")).is_err());
    }

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
