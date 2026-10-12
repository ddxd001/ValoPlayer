pub mod adapters;
pub mod db;
pub mod library;
pub mod media;
pub mod models;
#[cfg(feature="desktop")]
mod desktop {
use super::*;
use library::Library;
use models::*;
use std::{path::{Path,PathBuf},sync::{Arc,Mutex},time::{Duration,Instant}};
use tauri::{AppHandle,Emitter,Manager};

struct AppState { library: Arc<Mutex<Library>> }
fn scope_snapshot(app:&AppHandle,snapshot:&LibrarySnapshot) {
    let scope=app.asset_protocol_scope();
    for r in &snapshot.recordings {
        if Path::new(&r.path).is_file(){let _=scope.allow_file(&r.path);}
        if let Some(path)=&r.favorite_path {let _=scope.allow_file(path);}
        if let Some(path)=&r.thumbnail_path {let _=scope.allow_file(path);}
    }
}
async fn mutate(app:AppHandle,job:impl FnOnce(&mut Library)->Result<(),String>+Send+'static)->Result<LibrarySnapshot,String> {
    let shared=app.state::<AppState>().library.clone();
    let result=tauri::async_runtime::spawn_blocking(move|| {
        let mut lib=shared.lock().map_err(|_|"资料库暂时不可用".to_string())?;
        job(&mut lib)?; lib.snapshot()
    }).await.map_err(|e|e.to_string())?;
    if let Ok(snapshot)=&result {scope_snapshot(&app,snapshot);}
    // Even a partially failed batch must refresh the client to show successful operations.
    let _=app.emit("library-updated",()); result
}
#[tauri::command] async fn get_library(app:AppHandle)->Result<LibrarySnapshot,String> {
    let shared=app.state::<AppState>().library.clone();
    let snapshot=tauri::async_runtime::spawn_blocking(move||shared.lock().map_err(|_|"资料库暂时不可用".to_string())?.snapshot()).await.map_err(|e|e.to_string())??;
    scope_snapshot(&app,&snapshot);Ok(snapshot)
}
#[tauri::command] async fn add_source(app:AppHandle,path:String,name:String,kind:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|{lib.add_source(&path,&name,&kind)?;lib.scan()}).await}
#[tauri::command] async fn remove_source(app:AppHandle,id:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.remove_source(&id)).await}
#[tauri::command] async fn scan_sources(app:AppHandle)->Result<LibrarySnapshot,String> {let _=app.emit("scan-status","正在增量扫描…");let result=mutate(app.clone(),|lib|lib.scan()).await;let _=app.emit("scan-status",if result.is_ok(){"扫描完成"}else{"扫描失败"});result}
#[tauri::command] fn discover_sources()->Vec<String> {library::discover_sources()}
#[tauri::command] async fn import_files(app:AppHandle,paths:Vec<String>,metadata_path:Option<String>)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.import_files(&paths,metadata_path.as_deref())).await}
#[tauri::command] async fn set_favorites(app:AppHandle,ids:Vec<String>,mode:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.favorites(&ids,&mode)).await}
#[tauri::command] async fn create_category(app:AppHandle,name:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.create_category(&name)).await}
#[tauri::command] async fn delete_category(app:AppHandle,id:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.delete_category(&id)).await}
#[tauri::command] async fn set_categories(app:AppHandle,ids:Vec<String>,category_ids:Vec<String>)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.set_categories(&ids,&category_ids)).await}
#[tauri::command] async fn delete_recordings(app:AppHandle,ids:Vec<String>,target:String)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.delete_recordings(&ids,&target)).await}
#[tauri::command] async fn update_settings(app:AppHandle,settings:Settings)->Result<LibrarySnapshot,String> {mutate(app,move|lib|lib.db.save_settings(&settings)).await}
#[tauri::command] async fn get_environment(app:AppHandle)->Result<Environment,String> {
    let shared=app.state::<AppState>().library.clone();
    tauri::async_runtime::spawn_blocking(move||{let lib=shared.lock().map_err(|_|"资料库暂时不可用".to_string())?;let path=media::find_ffmpeg(&lib.db.settings()?.ffmpeg_path);Ok(Environment{ffmpeg_available:path.is_some(),ffmpeg_path:path.map(|p|p.to_string_lossy().to_string()).unwrap_or_default(),data_dir:lib.data_dir.to_string_lossy().to_string()})}).await.map_err(|e|e.to_string())?
}
#[tauri::command] async fn get_thumbnail(app:AppHandle,id:String,time:f64)->Result<String,String> {
    let shared=app.state::<AppState>().library.clone();
    let path=tauri::async_runtime::spawn_blocking(move||shared.lock().map_err(|_|"资料库暂时不可用".to_string())?.thumbnail(&id,time)).await.map_err(|e|e.to_string())??;
    let _=app.asset_protocol_scope().allow_file(&path);Ok(path.to_string_lossy().to_string())
}
#[tauri::command] async fn get_recording_media(app:AppHandle,id:String)->Result<Recording,String> {
    let shared=app.state::<AppState>().library.clone();
    tauri::async_runtime::spawn_blocking(move||{let lib=shared.lock().map_err(|_|"资料库暂时不可用".to_string())?;let mut r=lib.recording(&id)?;
        let input=lib.playback_path(&r)?;if let Ok(info)=media::probe(&input,&lib.db.settings()?.ffmpeg_path){r.duration=info.duration;r.width=info.width;r.height=info.height;r.fps=info.fps;lib.save(&r)?;}Ok(r)
    }).await.map_err(|e|e.to_string())?
}
fn export_context(app:&AppHandle,id:&str)->Result<(PathBuf,Recording,String),String> {
    let shared=app.state::<AppState>().library.clone();let lib=shared.lock().map_err(|_|"资料库暂时不可用".to_string())?;
    let r=lib.recording(id)?;Ok((lib.playback_path(&r)?,r,lib.db.settings()?.ffmpeg_path))
}
#[tauri::command] async fn export_clip(app:AppHandle,id:String,start:f64,end:f64,output_path:String,precise:bool)->Result<String,String> {
    let _=app.emit("operation-progress",serde_json::json!({"message":"正在导出 MP4…"}));
    tauri::async_runtime::spawn_blocking(move||{
        let (input,_,setting)=export_context(&app,&id)?;
        let output=PathBuf::from(&output_path);media::export_clip(&input,start,end,&output,precise,&setting)?;
        let _=app.emit("operation-progress",serde_json::json!({"message":"导出完成"}));Ok(output_path)
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command] async fn export_events(app:AppHandle,id:String,before:f64,after:f64,output_dir:String)->Result<Vec<String>,String> {
    if !before.is_finite()||!after.is_finite()||before<0.0||after<=0.0||before+after>300.0{return Err("事件前后截取范围需在 0–300 秒内，后段必须大于 0".into())}
    tauri::async_runtime::spawn_blocking(move||{
        let (input,r,setting)=export_context(&app,&id)?;
        let directory=PathBuf::from(output_dir);if !directory.is_dir(){return Err("请选择已存在的导出目录".into())}
        let info=media::probe(&input,&setting)?;let events:Vec<_>=r.events.iter().filter(|e|e.kind=="kill"&&e.time.is_finite()&&e.time>=0.0&&e.time<info.duration).collect();
        if events.is_empty(){return Err("该录像没有可验证的击杀时间点".into())}
        let mut outputs=Vec::new();
        let suffix=uuid::Uuid::new_v4().to_string();
        for (i,event) in events.iter().enumerate(){
            let output=directory.join(format!("kill-{}-{}-{}.mp4",&r.id[..8.min(r.id.len())],i+1,&suffix[..8]));
            let _=app.emit("operation-progress",serde_json::json!({"message":format!("正在导出击杀 {}/{}",i+1,events.len())}));
            media::export_clip(&input,(event.time-before).max(0.0),(event.time+after).min(info.duration),&output,true,&setting).map_err(|e|format!("已完成 {} 个片段；后续导出失败：{e}",outputs.len()))?;
            outputs.push(output.to_string_lossy().to_string());
        }
        let _=app.emit("operation-progress",serde_json::json!({"message":format!("已导出 {} 个击杀片段",outputs.len())}));Ok(outputs)
    }).await.map_err(|e|e.to_string())?
}
fn start_monitor(app:AppHandle) {
    std::thread::spawn(move|| {
        use notify::{Watcher,RecursiveMode,EventKind};
        let shared=app.state::<AppState>().library.clone();
        let (tx,rx)=std::sync::mpsc::channel();
        let mut watcher=notify::recommended_watcher(move|event:notify::Result<notify::Event>|{let _=tx.send(event);}).ok();
        let mut watched=Vec::<PathBuf>::new();let mut last_scan=Instant::now()-Duration::from_secs(60);let mut dirty_at=None::<Instant>;let mut last_paths=Instant::now()-Duration::from_secs(10);
        loop {
            let Ok(lib)=shared.try_lock() else {std::thread::sleep(Duration::from_millis(100));continue};
            let data_key=library::path_key(&lib.data_dir);let auto=lib.db.settings().map(|s|s.auto_scan).unwrap_or(false);
            let paths=if last_paths.elapsed()>Duration::from_secs(5){last_paths=Instant::now();lib.watch_paths()}else{watched.clone()};drop(lib);
            let relevant=|event:notify::Event|!matches!(event.kind,EventKind::Access(_))&&event.paths.iter().any(|p|{let key=library::path_key(p);key!=data_key&&!key.starts_with(&(data_key.clone()+"/"))});
            if let Ok(Ok(event))=rx.recv_timeout(Duration::from_secs(2)){if relevant(event){dirty_at=Some(Instant::now())}}
            for event in rx.try_iter().flatten(){if relevant(event){dirty_at=Some(Instant::now())}}
            if let Some(watcher)=watcher.as_mut(){if paths!=watched{for p in &watched{let _=watcher.unwatch(p);}for p in &paths{let _=watcher.watch(p,RecursiveMode::Recursive);}watched=paths;}}
            if auto && (last_scan.elapsed()>Duration::from_secs(45)||dirty_at.is_some_and(|t|t.elapsed()>Duration::from_secs(2))) {
                let _=app.emit("scan-status","正在自动更新…");
                if let Ok(mut lib)=shared.lock(){match lib.scan(){Ok(())=>{if let Ok(snapshot)=lib.snapshot(){scope_snapshot(&app,&snapshot);}let _=app.emit("library-updated",());},Err(e)=>{let _=app.emit("scan-status",e);}}}
                last_scan=Instant::now();dirty_at=None;let _=app.emit("scan-status","目录监测中");
            }
        }
    });
}
pub fn run() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).setup(|app| {
        let data_dir=app.path().app_local_data_dir()?;
        let lib=Library::open(data_dir).map_err(std::io::Error::other)?;
        if lib.db.sources().map_err(std::io::Error::other)?.is_empty(){for path in library::discover_sources(){let _=lib.add_source(&path,"","wegame");}}
        app.manage(AppState{library:Arc::new(Mutex::new(lib))});start_monitor(app.handle().clone());Ok(())
    }).invoke_handler(tauri::generate_handler![get_library,add_source,remove_source,scan_sources,discover_sources,import_files,set_favorites,create_category,delete_category,set_categories,delete_recordings,update_settings,get_environment,get_thumbnail,get_recording_media,export_clip,export_events])
        .run(tauri::generate_context!()).expect("无法启动 ValoPlayer");
}
}
#[cfg(feature="desktop")]
pub use desktop::run;
