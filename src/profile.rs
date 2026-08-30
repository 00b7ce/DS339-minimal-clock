use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::renderer::ClockStyle;

pub const DEFAULT_PROFILE_NAME: &str = "Default";
const CONFIG_DIRECTORY: &str = "jonsbo-ds339-minimal";
const PROFILE_FILE: &str = "profiles.toml";
const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileStore {
    pub version: u32,
    pub selected_profile: String,
    pub profiles: BTreeMap<String, ClockStyle>,
    pub start_on_launch: bool,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            selected_profile: DEFAULT_PROFILE_NAME.to_owned(),
            profiles: BTreeMap::from([(DEFAULT_PROFILE_NAME.to_owned(), ClockStyle::default())]),
            start_on_launch: false,
        }
    }
}

impl ProfileStore {
    fn normalize(mut self) -> Self {
        self.version = SCHEMA_VERSION;
        if self.profiles.is_empty() {
            self.profiles
                .insert(DEFAULT_PROFILE_NAME.to_owned(), ClockStyle::default());
        }
        if !self.profiles.contains_key(&self.selected_profile) {
            self.selected_profile = self
                .profiles
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_owned());
        }
        self
    }
}

pub struct LoadedProfiles {
    pub store: ProfileStore,
    pub warning: Option<String>,
}

pub struct ProfileRepository {
    path: PathBuf,
}

impl ProfileRepository {
    pub fn new() -> Result<Self, String> {
        let base = BaseDirs::new().ok_or_else(|| "config directory is unavailable".to_owned())?;
        Ok(Self::at_path(
            base.config_dir().join(CONFIG_DIRECTORY).join(PROFILE_FILE),
        ))
    }

    fn at_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<LoadedProfiles, String> {
        if !self.path.exists() {
            return Ok(LoadedProfiles {
                store: ProfileStore::default(),
                warning: None,
            });
        }

        match read_store(&self.path) {
            Ok(store) => Ok(LoadedProfiles {
                store: store.normalize(),
                warning: None,
            }),
            Err(primary_error) => {
                let backup = backup_path(&self.path);
                let store = read_store(&backup).map_err(|backup_error| {
                    format!(
                        "failed to load {} ({primary_error}); backup also failed ({backup_error})",
                        self.path.display()
                    )
                })?;
                Ok(LoadedProfiles {
                    store: store.normalize(),
                    warning: Some(format!(
                        "Recovered profiles from {} because the main file was invalid",
                        backup.display()
                    )),
                })
            }
        }
    }

    pub fn save(&self, store: &ProfileStore) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "profile path has no parent directory".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create profile directory {}: {error}",
                parent.display()
            )
        })?;

        if self.path.exists() {
            let backup = backup_path(&self.path);
            fs::copy(&self.path, &backup).map_err(|error| {
                format!(
                    "failed to back up profiles to {}: {error}",
                    backup.display()
                )
            })?;
        }

        let contents = toml::to_string_pretty(store)
            .map_err(|error| format!("failed to serialize profiles: {error}"))?;
        fs::write(&self.path, contents).map_err(|error| {
            format!(
                "failed to save profiles to {}: {error}",
                self.path.display()
            )
        })
    }
}

fn read_store(path: &Path) -> Result<ProfileStore, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("toml.bak")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_repository(name: &str) -> ProfileRepository {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ProfileRepository::at_path(
            std::env::temp_dir()
                .join(format!(
                    "jonsbo-ds339-minimal-{name}-{}-{unique}",
                    std::process::id()
                ))
                .join(PROFILE_FILE),
        )
    }

    fn clean(repository: &ProfileRepository) {
        if let Some(parent) = repository.path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn saves_and_loads_multiple_profiles() {
        let repository = temporary_repository("roundtrip");
        let mut store = ProfileStore::default();
        store.profiles.insert(
            "Large clock".to_owned(),
            ClockStyle {
                font_size: 180.0,
                ..ClockStyle::default()
            },
        );
        store.selected_profile = "Large clock".to_owned();

        repository.save(&store).unwrap();
        let loaded = repository.load().unwrap();

        assert_eq!(loaded.store.selected_profile, "Large clock");
        assert_eq!(loaded.store.profiles["Large clock"].font_size, 180.0);
        assert!(loaded.warning.is_none());
        clean(&repository);
    }

    #[test]
    fn recovers_from_the_backup_file() {
        let repository = temporary_repository("backup");
        let mut store = ProfileStore::default();
        repository.save(&store).unwrap();
        store.start_on_launch = true;
        repository.save(&store).unwrap();
        fs::write(repository.path(), "invalid = [").unwrap();

        let loaded = repository.load().unwrap();

        assert!(!loaded.store.start_on_launch);
        assert!(loaded.warning.is_some());
        clean(&repository);
    }
}
