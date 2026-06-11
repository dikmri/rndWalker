use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const APP_NAME: &str = "rndWalker";
pub const NUM_GROUPS: usize = 3;
pub const MAX_SUB_FOLDERS: usize = 4;
pub const DEFAULT_VOLUME: f32 = 0.10;
pub const DEFAULT_MULTIVIEW_VIDEO_SIZE: f32 = 320.0;
pub const MIN_MULTIVIEW_VIDEO_SIZE: f32 = 160.0;
pub const MAX_MULTIVIEW_VIDEO_SIZE: f32 = 720.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub mp4_folder_paths: Vec<Vec<String>>,
    pub mp3_folder_path: String,
    pub volume: f32,
    pub multiview_enabled: bool,
    pub multiview_video_size: f32,
    pub presets: BTreeMap<String, FolderPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FolderPreset {
    pub mp4_folder_paths: Vec<Vec<String>>,
    pub mp3_folder_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mp4_folder_paths: empty_folder_groups(),
            mp3_folder_path: String::new(),
            volume: DEFAULT_VOLUME,
            multiview_enabled: false,
            multiview_video_size: DEFAULT_MULTIVIEW_VIDEO_SIZE,
            presets: BTreeMap::new(),
        }
    }
}

impl Default for FolderPreset {
    fn default() -> Self {
        Self {
            mp4_folder_paths: empty_folder_groups(),
            mp3_folder_path: String::new(),
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
        for preset in self.presets.values_mut() {
            preset.mp4_folder_paths =
                normalize_folder_groups(std::mem::take(&mut preset.mp4_folder_paths));
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.mp4_folder_paths[0][0].trim().is_empty() && !self.mp3_folder_path.trim().is_empty()
    }
}

pub fn empty_folder_groups() -> Vec<Vec<String>> {
    vec![vec![String::new(); MAX_SUB_FOLDERS]; NUM_GROUPS]
}

pub fn normalize_folder_groups(mut groups: Vec<Vec<String>>) -> Vec<Vec<String>> {
    groups.truncate(NUM_GROUPS);
    while groups.len() < NUM_GROUPS {
        groups.push(Vec::new());
    }

    for group in &mut groups {
        group.truncate(MAX_SUB_FOLDERS);
        while group.len() < MAX_SUB_FOLDERS {
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
        assert!(groups.iter().all(|group| group.len() == MAX_SUB_FOLDERS));
        assert_eq!(groups[0][0], "a");
        assert_eq!(groups[1][1], "c");
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
}
