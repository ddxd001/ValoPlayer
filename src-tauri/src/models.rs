use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MomentEvent { pub id: String, pub kind: String, pub time: f64, pub label: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String, pub title: String, pub path: String, pub source_id: String,
    pub account: String, pub created_at: u64, pub duration: f64,
    pub width: u32, pub height: u32, pub fps: f64, pub size: u64,
    pub available: bool, pub favorite_mode: String, pub favorite_path: Option<String>,
    pub category_ids: Vec<String>, pub events: Vec<MomentEvent>,
    pub metadata_status: String, pub thumbnail_path: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source { pub id: String, pub name: String, pub path: String, pub kind: String, pub account: String, pub status: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category { pub id: String, pub name: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings { pub ffmpeg_path: String, pub continuous_play: bool, pub auto_scan: bool }
impl Default for Settings { fn default() -> Self { Self { ffmpeg_path: String::new(), continuous_play: true, auto_scan: true } } }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot { pub recordings: Vec<Recording>, pub sources: Vec<Source>, pub categories: Vec<Category>, pub settings: Settings, pub diagnostics: Vec<String> }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment { pub ffmpeg_available: bool, pub ffmpeg_path: String, pub data_dir: String }
