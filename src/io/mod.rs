//! File IO: GeoJSON loading (off the UI thread), style project files, and
//! portable settings stored next to the executable.

use crate::model::{parse_geojson, Dataset, StyleProject};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Result of a background parse, delivered to the UI thread.
pub type LoadResult = Result<(String, Dataset), String>;

/// Handle to an in-flight background load. Drop it to detach the thread
/// (the result is simply discarded).
pub struct LoadHandle {
    receiver: Receiver<LoadResult>,
}

impl LoadHandle {
    pub fn spawn(path: PathBuf) -> LoadHandle {
        let (tx, rx): (Sender<LoadResult>, Receiver<LoadResult>) = mpsc::channel();
        thread::spawn(move || {
            let result: LoadResult = (|| {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("could not read file: {e}"))?;
                let ds = parse_geojson(&text).map_err(|e| e.to_string())?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "untitled.geojson".into());
                Ok((name, ds))
            })();
            let _ = tx.send(result);
        });
        LoadHandle { receiver: rx }
    }

    /// Non-blocking poll for the result.
    pub fn try_recv(&self) -> Option<LoadResult> {
        self.receiver.try_recv().ok()
    }
}

/// Directory for portable settings: `<exe_dir>/config/`, created on demand.
/// Falls back to `./config` if the executable path cannot be determined
/// (e.g. in some test environments).
pub fn app_config_dir() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let cfg = dir.join("config");
    let _ = std::fs::create_dir_all(&cfg);
    cfg
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppSettings {
    pub last_dir: Option<String>,
}

impl AppSettings {
    fn path() -> PathBuf {
        app_config_dir().join("settings.json")
    }

    pub fn load() -> AppSettings {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("could not save settings: {e}"))
    }
}

/// Save a style project as a JSON project file.
pub fn save_style_project(style: &StyleProject, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(style).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("could not save style: {e}"))
}

/// Load a style project from a JSON project file.
pub fn load_style_project(path: &Path) -> Result<StyleProject, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read file: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("not a valid style project: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    #[test]
    fn style_project_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("style.json");
        let mut style = StyleProject::default();
        style.fill.color = Color::new(1, 2, 3, 200);
        save_style_project(&style, &path).unwrap();
        let back = load_style_project(&path).unwrap();
        assert_eq!(style, back);
    }

    #[test]
    fn load_style_project_rejects_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("style.json");
        std::fs::write(&path, "not json at all").unwrap();
        let err = load_style_project(&path).unwrap_err();
        assert!(err.contains("not a valid style project"));
    }

    #[test]
    fn background_load_reports_parse_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.geojson");
        std::fs::write(&path, "{ broken").unwrap();
        let handle = LoadHandle::spawn(path);
        let result = loop {
            if let Some(r) = handle.try_recv() {
                break r;
            }
        };
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not valid JSON"));
    }

    #[test]
    fn background_load_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.geojson");
        std::fs::write(
            &path,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]},"properties":{"a":1}}]}"#,
        )
        .unwrap();
        let handle = LoadHandle::spawn(path);
        let (name, ds) = loop {
            if let Some(r) = handle.try_recv() {
                break r.unwrap();
            }
        };
        assert_eq!(name, "ok.geojson");
        assert_eq!(ds.features.len(), 1);
    }
}
