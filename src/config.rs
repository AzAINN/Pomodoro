use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const THEMES: [&str; 3] = ["Mono", "Sage", "Amber"];
pub const ALERTS: [&str; 3] = ["Once", "Repeat", "Off"];
pub const SOUNDS: [&str; 15] = [
    "Bell",
    "Basso",
    "Blow",
    "Bottle",
    "Frog",
    "Funk",
    "Glass",
    "Hero",
    "Morse",
    "Ping",
    "Pop",
    "Purr",
    "Sosumi",
    "Submarine",
    "Tink",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub long_break_interval: u32,
    pub sound: String,
    pub theme: String,
    pub alert: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            sound: "Ping".into(),
            theme: "Mono".into(),
            alert: "Once".into(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("Focus", self.focus_minutes),
            ("Short break", self.short_break_minutes),
            ("Long break", self.long_break_minutes),
        ] {
            if !(1..=1440).contains(&value) {
                bail!("{name} must be between 1 and 1440 minutes");
            }
        }
        if !(1..=99).contains(&self.long_break_interval) {
            bail!("Long break interval must be between 1 and 99");
        }
        if !THEMES.contains(&self.theme.as_str())
            || !ALERTS.contains(&self.alert.as_str())
            || !SOUNDS.contains(&self.sound.as_str())
        {
            bail!("Choose a listed theme, alert, and sound");
        }
        Ok(())
    }

    /// Validate individual fields so one bad value doesn't discard the rest.
    pub fn load(path: &Path) -> Result<(Self, Option<String>)> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Self::default(), None));
            }
            Err(error) => return Err(error).context("Could not read settings"),
        };
        let raw: Value = match serde_json::from_slice(&bytes) {
            Ok(Value::Object(values)) => Value::Object(values),
            _ => {
                return Ok((
                    Self::default(),
                    Some("Settings could not be read; using defaults. Original file kept.".into()),
                ));
            }
        };
        let mut settings = Self::default();
        let mut invalid = false;
        for (name, target, max) in [
            ("focus_minutes", &mut settings.focus_minutes, 1440),
            (
                "short_break_minutes",
                &mut settings.short_break_minutes,
                1440,
            ),
            ("long_break_minutes", &mut settings.long_break_minutes, 1440),
            ("long_break_interval", &mut settings.long_break_interval, 99),
        ] {
            if let Some(value) = raw.get(name) {
                if let Some(n) = value.as_u64().filter(|n| (1..=max).contains(n)) {
                    *target = n as u32;
                } else {
                    invalid = true;
                }
            }
        }
        for (name, target, choices) in [
            ("theme", &mut settings.theme, THEMES.as_slice()),
            ("alert", &mut settings.alert, ALERTS.as_slice()),
            ("sound", &mut settings.sound, SOUNDS.as_slice()),
        ] {
            if let Some(value) = raw.get(name) {
                if let Some(s) = value.as_str().filter(|s| choices.contains(s)) {
                    *target = s.into();
                } else {
                    invalid = true;
                }
            }
        }
        Ok((
            settings,
            invalid.then(|| {
                "Invalid settings replaced with defaults; original file kept until you save.".into()
            }),
        ))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path.parent().context("Settings path has no parent")?;
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        writeln!(file)?;
        file.as_file().sync_all()?;
        file.persist(path)
            .context("Could not replace settings file")?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub data: PathBuf,
    pub config: PathBuf,
}

impl Paths {
    pub fn discover(override_dir: Option<PathBuf>) -> Result<Self> {
        if let Some(dir) = override_dir.or_else(|| env::var_os("POMO_HOME").map(PathBuf::from)) {
            return Ok(Self {
                config: dir.join("config.json"),
                data: dir,
            });
        }
        let home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .context("Home directory unavailable; set POMO_HOME")?;
        let (data, config) = if cfg!(target_os = "macos") {
            let path = home.join("Library/Application Support/pomo");
            (path.clone(), path)
        } else if cfg!(windows) {
            let path = env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Local"))
                .join("pomo/pomo");
            (path.clone(), path)
        } else {
            let data = xdg("XDG_DATA_HOME", home.join(".local/share")).join("pomo");
            let config = xdg("XDG_CONFIG_HOME", home.join(".config")).join("pomo");
            (data, config)
        };
        Ok(Self {
            data,
            config: config.join("config.json"),
        })
    }

    pub fn database(&self) -> PathBuf {
        self.data.join("pomo.db")
    }
}

fn xdg(name: &str, fallback: PathBuf) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_and_invalid_fields_are_handled_individually() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"focus_minutes":50,"short_break_minutes":true,"long_break_interval":0,"theme":"Sage","sound":"../../bad"}"#).unwrap();
        let (settings, warning) = Settings::load(&path).unwrap();
        assert_eq!(settings.focus_minutes, 50);
        assert_eq!(settings.short_break_minutes, 5);
        assert_eq!(settings.long_break_interval, 4);
        assert_eq!(settings.theme, "Sage");
        assert_eq!(settings.sound, "Ping");
        assert_eq!(settings.alert, "Once");
        assert!(warning.is_some());
    }

    #[test]
    fn corrupt_config_is_preserved_and_save_is_validated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "bad json").unwrap();
        let (mut settings, warning) = Settings::load(&path).unwrap();
        assert!(warning.is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), "bad json");
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap().0, settings);
        settings.focus_minutes = 0;
        assert!(settings.save(&path).is_err());
        assert_eq!(Settings::load(&path).unwrap().0.focus_minutes, 25);
    }
}
