use crate::config::{AppSettings, NUM_GROUPS};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MediaLibrary {
    pub mp4_sets: Vec<Vec<PathBuf>>,
    pub mp3_files: Vec<PathBuf>,
}

impl Default for MediaLibrary {
    fn default() -> Self {
        Self {
            mp4_sets: vec![Vec::new(); NUM_GROUPS],
            mp3_files: Vec::new(),
        }
    }
}

impl MediaLibrary {
    pub fn from_settings(settings: &AppSettings) -> Self {
        let mut library = Self::default();

        for group_index in 0..NUM_GROUPS {
            let mut files = Vec::new();
            for folder in &settings.mp4_folder_paths[group_index] {
                let folder = folder.trim();
                if !folder.is_empty() {
                    files.extend(scan_folder(folder, "mp4"));
                }
            }
            library.mp4_sets[group_index] = files;
        }

        if !settings.mp3_folder_path.trim().is_empty() {
            library.mp3_files = scan_folder(&settings.mp3_folder_path, "mp3");
        }

        library
    }

    pub fn active_videos(&self, active_folder: Option<usize>) -> Vec<PathBuf> {
        match active_folder {
            Some(index) => self.mp4_sets.get(index).cloned().unwrap_or_default(),
            None => self.mp4_sets.iter().flatten().cloned().collect(),
        }
    }

    pub fn active_video_count(&self, active_folder: Option<usize>) -> usize {
        match active_folder {
            Some(index) => self.mp4_sets.get(index).map_or(0, Vec::len),
            None => self.mp4_sets.iter().map(Vec::len).sum(),
        }
    }

    pub fn non_empty_group_count(&self) -> usize {
        self.mp4_sets
            .iter()
            .filter(|group| !group.is_empty())
            .count()
    }
}

pub fn scan_folder(folder: impl AsRef<Path>, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder.as_ref()) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect();

    files.sort_by_cached_key(|path| path.file_name().map(|name| name.to_os_string()));
    files
}

pub fn choose_random_path(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        None
    } else {
        Some(paths[fastrand::usize(..paths.len())].clone())
    }
}

pub fn choose_random_path_avoiding_recent(
    paths: &[PathBuf],
    recent_paths: &[PathBuf],
    max_recent: usize,
) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }

    let limit = recent_paths.len().min(max_recent);
    for recent_count in (1..=limit).rev() {
        let candidates: Vec<PathBuf> = paths
            .iter()
            .filter(|path| {
                !recent_paths
                    .iter()
                    .take(recent_count)
                    .any(|recent| recent == *path)
            })
            .cloned()
            .collect();

        if !candidates.is_empty() {
            return choose_random_path(&candidates);
        }
    }

    choose_random_path(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn avoiding_recent_excludes_last_three_when_possible() {
        let paths = vec![path("a.mp4"), path("b.mp4"), path("c.mp4"), path("d.mp4")];
        let recent = vec![path("c.mp4"), path("b.mp4"), path("a.mp4")];

        assert_eq!(
            choose_random_path_avoiding_recent(&paths, &recent, 3),
            Some(path("d.mp4"))
        );
    }

    #[test]
    fn avoiding_recent_relaxes_exclusion_for_small_libraries() {
        let paths = vec![path("a.mp4"), path("b.mp4")];
        let recent = vec![path("a.mp4"), path("b.mp4")];

        assert_eq!(
            choose_random_path_avoiding_recent(&paths, &recent, 3),
            Some(path("b.mp4"))
        );
    }
}
