use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

pub const APP_NAME: &str = "rndWalker";
pub const DEFAULT_GROUPS: usize = 3;
pub const NUM_GROUPS: usize = 9;
pub const DEFAULT_VOLUME: f32 = 0.10;
pub const DEFAULT_MULTIVIEW_VIDEO_SIZE: f32 = 320.0;
pub const MIN_MULTIVIEW_VIDEO_SIZE: f32 = 160.0;
pub const MAX_MULTIVIEW_VIDEO_SIZE: f32 = 720.0;
pub const PRESET_FUNCTION_KEYS: [u8; 10] = [1, 2, 3, 4, 6, 7, 8, 9, 10, 12];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub mp4_folder_paths: Vec<Vec<String>>,
    pub mp3_folder_path: String,
    pub volume: f32,
    pub multiview_enabled: bool,
    pub multiview_video_size: f32,
    pub numpad_folder_switching: bool,
    pub presets: BTreeMap<String, FolderPreset>,
    pub active_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FolderPreset {
    pub mp4_folder_paths: Vec<Vec<String>>,
    pub mp3_folder_path: String,
    pub hotkey: Option<u8>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mp4_folder_paths: empty_folder_groups(),
            mp3_folder_path: String::new(),
            volume: DEFAULT_VOLUME,
            multiview_enabled: false,
            multiview_video_size: DEFAULT_MULTIVIEW_VIDEO_SIZE,
            numpad_folder_switching: false,
            presets: BTreeMap::new(),
            active_preset: None,
        }
    }
}

impl Default for FolderPreset {
    fn default() -> Self {
        Self {
            mp4_folder_paths: empty_folder_groups(),
            mp3_folder_path: String::new(),
            hotkey: None,
        }
    }
}

impl AppSettings {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read settings from {}", path.display()))?;
        let mut settings: Self = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse settings from {}", path.display()))?;
        settings.normalize();
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut normalized = self.clone();
        normalized.normalize();
        let contents = serde_json::to_string_pretty(&normalized)?;
        fs::write(&path, contents)
            .with_context(|| format!("failed to write settings to {}", path.display()))?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        self.mp4_folder_paths = normalize_folder_groups(std::mem::take(&mut self.mp4_folder_paths));
        self.volume = self.volume.clamp(0.0, 1.0);
        self.multiview_video_size = self
            .multiview_video_size
            .clamp(MIN_MULTIVIEW_VIDEO_SIZE, MAX_MULTIVIEW_VIDEO_SIZE);
        let mut used_hotkeys = HashSet::new();
        for preset in self.presets.values_mut() {
            preset.mp4_folder_paths =
                normalize_folder_groups(std::mem::take(&mut preset.mp4_folder_paths));
            if preset.hotkey.is_some_and(|key| {
                !PRESET_FUNCTION_KEYS.contains(&key) || !used_hotkeys.insert(key)
            }) {
                preset.hotkey = None;
            }
        }
        if self
            .active_preset
            .as_ref()
            .is_some_and(|name| !self.presets.contains_key(name))
        {
            self.active_preset = None;
        }
    }

    pub fn restore_active_preset(&mut self) -> bool {
        if self.active_preset.is_some() {
            return false;
        }

        let Some(name) = self.presets.iter().find_map(|(name, preset)| {
            (preset.mp4_folder_paths == self.mp4_folder_paths
                && preset.mp3_folder_path == self.mp3_folder_path)
                .then(|| name.clone())
        }) else {
            return false;
        };

        self.active_preset = Some(name);
        true
    }

    pub fn is_configured(&self) -> bool {
        !self.mp4_folder_paths[0][0].trim().is_empty() && !self.mp3_folder_path.trim().is_empty()
    }
}

pub fn empty_folder_groups() -> Vec<Vec<String>> {
    vec![vec![String::new()]; NUM_GROUPS]
}

pub fn normalize_folder_groups(mut groups: Vec<Vec<String>>) -> Vec<Vec<String>> {
    groups.truncate(NUM_GROUPS);
    while groups.len() < NUM_GROUPS {
        groups.push(Vec::new());
    }
    for group in &mut groups {
        while group.len() > 1 && group.last().is_some_and(|s| s.trim().is_empty()) {
            group.pop();
        }
        if group.is_empty() {
            group.push(String::new());
        }
    }
    groups
}

pub fn config_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("dev", "dikmri", APP_NAME)
        .context("failed to locate a platform config directory")?;
    Ok(project_dirs.config_dir().join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_missing_groups_and_slots() {
        let groups = normalize_folder_groups(vec![vec!["a".into()], vec!["b".into(), "c".into()]]);

        assert_eq!(groups.len(), NUM_GROUPS);
        assert_eq!(groups[0], vec!["a"]);
        assert_eq!(groups[1], vec!["b", "c"]);
        assert_eq!(groups[2], vec![""]);
    }

    #[test]
    fn normalize_clamps_multiview_video_size() {
        let mut settings = AppSettings {
            multiview_video_size: 9999.0,
            ..AppSettings::default()
        };

        settings.normalize();

        assert_eq!(settings.multiview_video_size, MAX_MULTIVIEW_VIDEO_SIZE);
    }

    #[test]
    fn normalize_keeps_only_unique_supported_preset_hotkeys() {
        let mut settings = AppSettings::default();
        settings.presets.insert(
            "a".into(),
            FolderPreset {
                hotkey: Some(1),
                ..FolderPreset::default()
            },
        );
        settings.presets.insert(
            "b".into(),
            FolderPreset {
                hotkey: Some(1),
                ..FolderPreset::default()
            },
        );
        settings.presets.insert(
            "invalid".into(),
            FolderPreset {
                hotkey: Some(5),
                ..FolderPreset::default()
            },
        );

        settings.normalize();

        assert_eq!(settings.presets["a"].hotkey, Some(1));
        assert_eq!(settings.presets["b"].hotkey, None);
        assert_eq!(settings.presets["invalid"].hotkey, None);
    }

    #[test]
    fn restore_active_preset_matches_current_folders() {
        let mut settings = AppSettings::default();
        settings.mp4_folder_paths[0][0] = "videos".into();
        settings.mp3_folder_path = "music".into();
        settings.presets.insert(
            "work".into(),
            FolderPreset {
                mp4_folder_paths: settings.mp4_folder_paths.clone(),
                mp3_folder_path: settings.mp3_folder_path.clone(),
                ..FolderPreset::default()
            },
        );

        assert!(settings.restore_active_preset());
        assert_eq!(settings.active_preset.as_deref(), Some("work"));
    }
}
