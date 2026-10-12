# Internal implementation contract

Vue 3 + TypeScript frontend invokes Tauri 2 commands. App-owned SQLite is writable; official metadata stores must only be opened read-only. No game process access.

DTOs use camelCase. Recording: `id, title, path, sourceId, account, createdAt` (epoch seconds), `duration` (seconds), `width, height, fps, size, available, favoriteMode` (`none|index|copy`), `favoritePath` (nullable), `categoryIds` (string[]), `events` (MomentEvent[]), `metadataStatus, thumbnailPath` (nullable). Event: `id, kind` (`kill|death|assist|round|unknown`), `time` (seconds), `label`. Source: `id, name, path, kind` (`wegame|import`), `account, status`. Category: `id,name`. Settings: `ffmpegPath, continuousPlay, autoScan`. Snapshot: `recordings,sources,categories,settings,diagnostics`.

Commands:
- get_library() -> Snapshot
- add_source(path,name,kind) -> Snapshot
- remove_source(id) -> Snapshot
- scan_sources() -> Snapshot
- discover_sources() -> string[]
- import_files(paths,metadataPath?) -> Snapshot
- set_favorites(ids,mode) -> Snapshot
- create_category(name) -> Snapshot
- delete_category(id) -> Snapshot
- set_categories(ids,categoryIds) -> Snapshot
- delete_recordings(ids,target) -> Snapshot; target original|copy|index; disk deletion always uses Recycle Bin
- update_settings(settings) -> Snapshot
- get_environment() -> {ffmpegAvailable:boolean, ffmpegPath:string, dataDir:string}
- get_thumbnail(id,time) -> string (local image path)
- export_clip(id,start,end,outputPath,precise) -> string
- export_events(id,before,after,outputDir) -> string[]

Events emitted: `library-updated` (rescan UI), `scan-status` (string), `operation-progress` ({message:string}). Native dialog plugin for file/folder selection. Local videos served via Tauri asset protocol.

Backend modules: models.rs (DTOs), db.rs/library.rs/lib.rs (root), media.rs (media agent), adapters/mod.rs (ACLOS agent). Adapter interface `pub fn inspect_video(path: &Path, source_root: &Path, explicit_metadata: Option<&Path>) -> Metadata`, `pub fn inspect_source(root: &Path) -> Vec<String>`. Metadata fields: title: Option<String>, account: Option<String>, duration: Option<f64>, events: Vec<MomentEvent>, status: String, diagnostics: Vec<String>. Adapter can export other helpers. media interface: `probe(path:&Path, ffmpeg_setting:&str)->Result<MediaInfo,String>`, `thumbnail(input:&Path,time:f64,output:&Path,setting:&str)->Result<(),String>`, `export_clip(input:&Path,start:f64,end:f64,output:&Path,precise:bool,setting:&str)->Result<(),String>`, `find_ffmpeg(setting:&str)->Option<PathBuf>`. MediaInfo defined in media.rs with duration,width,height,fps. Modules do not access Tauri State directly.
