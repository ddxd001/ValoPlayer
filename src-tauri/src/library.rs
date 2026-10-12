use crate::{adapters, db::Database, media, models::*};
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;
use walkdir::WalkDir;

pub struct Library { pub db: Database, pub data_dir: PathBuf, pub diagnostics: Vec<String> }
pub fn path_key(path:&Path)->String { path.to_string_lossy().replace('\\',"/").trim_start_matches("//?/").to_lowercase() }
pub fn is_video(path:&Path)->bool { matches!(path.extension().and_then(|x|x.to_str()).map(|x|x.to_ascii_lowercase()).as_deref(),Some("mp4"|"mkv"|"mov"|"webm"|"avi"|"m4v")) }
fn epoch(time:SystemTime)->u64 { time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn canonical(path:&Path)->Result<PathBuf,String> { fs::canonicalize(path).map_err(|e|format!("无法访问 {}：{e}",path.display())) }
pub fn discover_sources()->Vec<String> {
    let mut found=Vec::new();
    if let Some(appdata)=std::env::var_os("APPDATA") {for folder in ["WonderfulDb","WeGameWonderfulDb"] { let p=PathBuf::from(&appdata).join("ACLOS").join(folder); if p.is_dir(){found.push(p.to_string_lossy().to_string())} }}
    found
}
impl Library {
    pub fn open(data_dir:PathBuf)->Result<Self,String> {
        fs::create_dir_all(data_dir.join("favorites")).map_err(|e|e.to_string())?;
        fs::create_dir_all(data_dir.join("thumbnails")).map_err(|e|e.to_string())?;
        let data_dir=canonical(&data_dir)?;
        let db=Database::open(&data_dir.join("library.sqlite3"))?;
        Ok(Self{db,data_dir,diagnostics:Vec::new()})
    }
    pub fn snapshot(&self)->Result<LibrarySnapshot,String> {
        let mut recordings=self.db.recordings()?;
        for r in &mut recordings {r.available=Path::new(&r.path).is_file(); if r.favorite_path.as_ref().is_some_and(|p|!Path::new(p).is_file()) {r.favorite_path=None; if r.favorite_mode=="copy" {r.favorite_mode="index".into();}}}
        recordings.sort_by(|a,b|b.created_at.cmp(&a.created_at));
        Ok(LibrarySnapshot{recordings,sources:self.db.sources()?,categories:self.db.categories()?,settings:self.db.settings()?,diagnostics:self.diagnostics.clone()})
    }
    pub fn recording(&self,id:&str)->Result<Recording,String> {self.db.recording_by_id(id)}
    pub fn save(&self,r:&Recording)->Result<(),String> {self.db.save_recording(r,&path_key(Path::new(&r.path)),"")}
    pub fn add_source(&self,path:&str,name:&str,kind:&str)->Result<(),String> {
        if !["wegame","import"].contains(&kind) {return Err("目录类型无效".into())}
        let path=canonical(Path::new(path))?;
        if !path.is_dir(){return Err("请选择目录".into())}
        if path.starts_with(&self.data_dir){return Err("收藏和缓存目录不能作为录像来源".into())}
        if self.db.sources()?.iter().any(|s|path_key(Path::new(&s.path))==path_key(&path)){return Ok(())}
        self.db.save_source(&Source{id:Uuid::new_v4().to_string(),name:if name.trim().is_empty(){path.file_name().unwrap_or_default().to_string_lossy().to_string()}else{name.trim().to_string()},path:path.to_string_lossy().to_string(),kind:kind.into(),account:"多账号".into(),status:"待扫描".into()})
    }
    pub fn remove_source(&self,id:&str)->Result<(),String> { self.db.remove_source(id) }
    pub fn index_video(&mut self,path:&Path,source:&Source,explicit:Option<&Path>,force:bool,source_stamp:&str)->Result<bool,String> {
        if !is_video(path){return Ok(false)}
        // Official metadata may reference recordings that have already been removed.
        let resolved=if path.exists(){canonical(path)?}else{path.to_path_buf()};
        if resolved.starts_with(&self.data_dir){return Ok(false)}
        let key=path_key(&resolved); if self.db.is_ignored(&key)?{return Ok(false)}
        let old=self.db.recording_by_key(&key)?;
        let stat=fs::metadata(&resolved).ok();
        let stamp=format!("{}:{}:{}",stat.as_ref().map(|s|s.len()).unwrap_or(0),stat.as_ref().and_then(|s|s.modified().ok()).map(|s|s.duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()).unwrap_or(0),source_stamp);
        let video_changed=old.as_ref().and_then(|r|self.db.stamp(&r.id).ok()).map(|s|s.split(':').take(2).collect::<Vec<_>>().join(":"))!=Some(stamp.split(':').take(2).collect::<Vec<_>>().join(":"));
        if !force {if let Some(r)=&old {if self.db.stamp(&r.id)?==stamp{return Ok(false)}}}
        let mut meta=adapters::inspect_video(&resolved,Path::new(&source.path),explicit);
        if explicit.is_none(){
            // Multiple accounts/variants can reference the same file. Prefer the
            // verified richer adapter regardless of directory insertion order.
            for other in self.db.sources()?.into_iter().filter(|s|s.id!=source.id){
                let candidate=adapters::inspect_video(&resolved,Path::new(&other.path),None);
                let score=|m:&adapters::Metadata| (match m.status.as_str(){"exchange-v1"=>4,"aclos-aes256"|"plain-json"=>3,"aclos-wegame-aes128"=>2,_=>0},m.events.len());
                if score(&candidate)>score(&meta){meta=candidate;}
            }
        }
        self.diagnostics.extend(meta.diagnostics);
        let settings=self.db.settings()?;
        // ACLOS already provides duration. Defer detailed media probing to thumbnail/player use.
        let info=if stat.is_some() && meta.duration.is_none(){media::probe(&resolved,&settings.ffmpeg_path).ok()}else{None};
        let mut r=old.unwrap_or(Recording{id:Uuid::new_v4().to_string(),title:resolved.file_stem().unwrap_or_default().to_string_lossy().to_string(),path:resolved.to_string_lossy().to_string(),source_id:source.id.clone(),account:source.account.clone(),created_at:stat.as_ref().and_then(|s|s.created().or_else(|_|s.modified()).ok()).map(epoch).unwrap_or(0),duration:0.0,width:0,height:0,fps:30.0,size:0,available:stat.is_some(),favorite_mode:"none".into(),favorite_path:None,category_ids:vec![],events:vec![],metadata_status:"未匹配".into(),thumbnail_path:None});
        if let Some(title)=meta.title {r.title=title;}
        if let Some(account)=meta.account {r.account=account;}
        r.duration=info.as_ref().map(|i|i.duration).or(meta.duration).unwrap_or(r.duration);
        if let Some(info)=info {r.width=info.width;r.height=info.height;r.fps=info.fps;}
        r.size=stat.as_ref().map(|s|s.len()).unwrap_or(0);r.available=stat.is_some();
        r.events=meta.events;
        r.metadata_status=meta.status;
        if video_changed {if let Some(thumbnail)=r.thumbnail_path.take(){let _=fs::remove_file(thumbnail);}}
        self.db.save_recording(&r,&key,&stamp)?; Ok(true)
    }
    pub fn scan(&mut self)->Result<(),String> {
        self.diagnostics.clear(); let mut seen=HashSet::new();
        for mut source in self.db.sources()? {
            let root=PathBuf::from(&source.path);
            if !root.is_dir(){source.status="目录不可用".into();self.db.save_source(&source)?;continue}
            let mut paths=Vec::new();
            for entry in WalkDir::new(&root).follow_links(false).max_depth(12).into_iter().filter_entry(|e|!e.path().starts_with(&self.data_dir)) {
                match entry {Ok(e) if e.file_type().is_file() && is_video(e.path())=>paths.push(e.path().to_path_buf()),Err(e)=>self.diagnostics.push(format!("扫描 {}：{e}",source.name)),_=>{}}
            }
            let (references,diagnostics)=adapters::enumerate_source_videos(&root);
            paths.extend(references); self.diagnostics.extend(diagnostics);
            let mut source_stamps:Vec<_>=self.db.sources()?.iter().map(|s|format!("{}:{}",s.id,metadata_stamp(Path::new(&s.path)))).collect();source_stamps.sort();
            let source_stamp=source_stamps.join("|");
            let mut changed=0; let mut count=0;
            for path in paths {if seen.insert(path_key(&path)) {count+=1; match self.index_video(&path,&source,None,false,&source_stamp) {Ok(true)=>changed+=1,Err(e)=>self.diagnostics.push(e),_=>{}}}}
            source.status=format!("{count} 个录像 · 更新 {changed} 个"); self.db.save_source(&source)?;
        }
        for mut r in self.db.recordings()? {let exists=Path::new(&r.path).is_file();if exists!=r.available{r.available=exists;self.save(&r)?;}}
        self.diagnostics.sort();self.diagnostics.dedup();self.diagnostics.truncate(60); Ok(())
    }
    pub fn import_files(&mut self,paths:&[String],metadata:Option<&str>)->Result<(),String> {
        let explicit=metadata.map(Path::new);
        if let Some(p)=explicit {if !p.exists(){return Err("元数据文件不存在".into())}}
        let mut sources:HashMap<String,Source>=HashMap::new();
        // File imports are tracked individually; they never cause an entire parent directory import.
        for path in paths {let p=canonical(Path::new(path))?;if !p.is_file()||!is_video(&p){return Err("请选择受支持的视频文件".into())} let parent=p.parent().ok_or("目录无效")?;let key=path_key(parent);
            let source=sources.entry(key).or_insert_with(||Source{id:format!("files:{}",Uuid::new_v4()),name:"外部视频".into(),path:parent.to_string_lossy().to_string(),kind:"import".into(),account:"外部导入".into(),status:"单文件导入".into()}).clone();
            self.db.unignore(&path_key(&p))?;self.index_video(&p,&source,explicit,true,"")?;
        } Ok(())
    }
    pub fn favorites(&self,ids:&[String],mode:&str)->Result<(),String> {
        if !["none","index","copy"].contains(&mode){return Err("收藏类型无效".into())}
        let records=ids.iter().map(|id|self.recording(id)).collect::<Result<Vec<_>,_>>()?;
        for mut r in records {
            if mode=="copy" && r.favorite_path.as_ref().is_none_or(|p|!Path::new(p).is_file()) {
                let input=self.playback_path(&r)?; let ext=input.extension().and_then(|e|e.to_str()).unwrap_or("mp4");
                let output=self.data_dir.join("favorites").join(format!("{}.{}",r.id,ext));
                if output.exists(){return Err("收藏文件已存在，请先移除永久副本后重试".into())}
                let temp=output.with_extension(format!("{ext}.partial"));
                if let Err(e)=fs::copy(&input,&temp){let _=fs::remove_file(&temp);return Err(format!("永久副本创建失败：{e}"))}
                if let Err(e)=fs::rename(&temp,&output){let _=fs::remove_file(&temp);return Err(e.to_string())}
                r.favorite_path=Some(output.to_string_lossy().to_string());
            }
            // Changing favorite mode never silently destroys an existing permanent copy.
            r.favorite_mode=mode.into();self.save(&r)?;
        } Ok(())
    }
    pub fn create_category(&self,name:&str)->Result<(),String> {
        let name=name.trim();if name.is_empty()||name.chars().count()>40{return Err("分类名称需为 1–40 个字符".into())}
        if self.db.categories()?.iter().any(|c|c.name==name){return Err("分类名称已存在".into())}
        self.db.save_category(&Category{id:Uuid::new_v4().to_string(),name:name.into()})
    }
    pub fn set_categories(&self,ids:&[String],category_ids:&[String])->Result<(),String> {
        let valid:HashSet<_>=self.db.categories()?.into_iter().map(|c|c.id).collect();
        if category_ids.iter().any(|id|!valid.contains(id)){return Err("分类已不存在，请刷新后重试".into())}
        for id in ids {let mut r=self.recording(id)?;r.category_ids=category_ids.to_vec();r.category_ids.sort();r.category_ids.dedup();self.save(&r)?;}Ok(())
    }
    pub fn delete_category(&self,id:&str)->Result<(),String> {self.db.remove_category(id)?;for mut r in self.db.recordings()? {r.category_ids.retain(|c|c!=id);self.save(&r)?;}Ok(())}
    pub fn delete_recordings(&self,ids:&[String],target:&str)->Result<(),String> {
        if !["original","copy","index"].contains(&target){return Err("删除目标无效".into())}
        let records=ids.iter().map(|id|self.recording(id)).collect::<Result<Vec<_>,_>>()?;
        let mut failures=Vec::new();
        for mut r in records {
            if target=="index" {
                if r.favorite_path.as_ref().is_some_and(|p|Path::new(p).is_file()){failures.push(format!("{}：请先移除永久副本，避免副本失去索引",r.title));continue}
                self.db.forget(&r.id,&path_key(Path::new(&r.path)))?; continue
            }
            let candidate=if target=="original"{Some(r.path.clone())}else{r.favorite_path.clone()};
            if let Some(candidate)=candidate {let p=Path::new(&candidate);if p.exists(){
                if !is_video(p){failures.push("不是录像文件，已拒绝删除".into());continue}
                let resolved=match canonical(p){Ok(p)=>p,Err(e)=>{failures.push(e);continue}};
                if target=="copy" && !resolved.starts_with(canonical(&self.data_dir.join("favorites"))?){failures.push("副本不在本应用收藏目录，已拒绝删除".into());continue}
                if let Err(e)=trash::delete(&resolved){failures.push(format!("{}：无法移至回收站（未执行永久删除）：{e}",r.title));continue}
            }}
            if target=="original"{r.available=false;}else{r.favorite_path=None;if r.favorite_mode=="copy"{r.favorite_mode="index".into()}}
            self.save(&r)?;
        }
        if failures.is_empty(){Ok(())}else{Err(failures.join("\n"))}
    }
    pub fn playback_path(&self,r:&Recording)->Result<PathBuf,String> {
        if let Some(copy)=&r.favorite_path{if Path::new(copy).is_file(){return canonical(Path::new(copy))}}
        canonical(Path::new(&r.path)).and_then(|p|if p.is_file(){Ok(p)}else{Err("录像文件不存在".into())})
    }
    pub fn watch_paths(&self)->Vec<PathBuf> {
        let mut paths:Vec<PathBuf>=self.db.sources().unwrap_or_default().into_iter().map(|s|PathBuf::from(s.path)).collect();
        for r in self.db.recordings().unwrap_or_default(){if let Some(p)=Path::new(&r.path).parent(){if p.is_dir(){paths.push(p.to_path_buf())}}}
        paths.sort();paths.dedup();paths
    }
    pub fn thumbnail(&mut self,id:&str,time:f64)->Result<PathBuf,String> {
        if !time.is_finite()||time<0.0{return Err("缩略图时间无效".into())}
        let mut r=self.recording(id)?;let input=self.playback_path(&r)?;
        if r.width==0 {if let Ok(info)=media::probe(&input,&self.db.settings()?.ffmpeg_path){r.width=info.width;r.height=info.height;r.fps=info.fps;r.duration=info.duration;self.save(&r)?;}}
        let time=if r.duration>0.0{time.min((r.duration-0.1).max(0.0))}else{time};
        let bucket=(time/2.0).floor() as u64;
        let stamp=fs::metadata(&input).and_then(|m|m.modified()).map(epoch).unwrap_or(0);
        let output=self.data_dir.join("thumbnails").join(format!("{}-{stamp}-{bucket}.jpg",r.id));
        if !output.is_file(){media::thumbnail(&input,bucket as f64*2.0,&output,&self.db.settings()?.ffmpeg_path)?;}
        if time<=2.0 {r.thumbnail_path=Some(output.to_string_lossy().to_string());self.save(&r)?;}
        Ok(output)
    }
}
fn metadata_stamp(root:&Path)->String {
    let mut result=0_u128;
    for e in WalkDir::new(root).max_depth(4).follow_links(false).into_iter().flatten(){if e.file_type().is_file()&&!is_video(e.path()){if let Ok(stat)=e.metadata(){result=result.wrapping_add(stat.len() as u128).wrapping_add(stat.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_nanos()).unwrap_or(0));}}}
    result.to_string()
}

#[cfg(test)] mod tests {
    use super::*;
    fn fixture()->(tempfile::TempDir,Library,PathBuf) {let dir=tempfile::tempdir().unwrap();let video=dir.path().join("external/中文.mp4");fs::create_dir_all(video.parent().unwrap()).unwrap();fs::write(&video,b"test video").unwrap();let app=Library::open(dir.path().join("app")).unwrap();(dir,app,video)}
    #[test] fn imports_dedupe_and_preserve_favorites_when_original_disappears(){let (_dir,mut lib,path)=fixture();lib.import_files(&[path.to_string_lossy().into()],None).unwrap();lib.import_files(&[path.to_string_lossy().into()],None).unwrap();assert_eq!(lib.snapshot().unwrap().recordings.len(),1);let id=lib.snapshot().unwrap().recordings[0].id.clone();lib.favorites(&[id.clone()],"copy").unwrap();fs::remove_file(path).unwrap();let r=lib.snapshot().unwrap().recordings.remove(0);assert!(!r.available);assert!(lib.playback_path(&r).unwrap().is_file());assert_eq!(r.favorite_mode,"copy");}
    #[test] fn forgetting_index_survives_rescan_but_explicit_import_restores(){let (_dir,mut lib,path)=fixture();lib.add_source(path.parent().unwrap().to_str().unwrap(),"test","import").unwrap();lib.scan().unwrap();let id=lib.snapshot().unwrap().recordings[0].id.clone();lib.delete_recordings(&[id],"index").unwrap();lib.scan().unwrap();assert!(lib.snapshot().unwrap().recordings.is_empty());lib.import_files(&[path.to_string_lossy().into()],None).unwrap();assert_eq!(lib.snapshot().unwrap().recordings.len(),1);}
    #[test] fn category_validation_and_removal(){let (_dir,mut lib,path)=fixture();lib.import_files(&[path.to_string_lossy().into()],None).unwrap();let id=lib.snapshot().unwrap().recordings[0].id.clone();assert!(lib.set_categories(&[id.clone()],&["missing".into()]).is_err());lib.create_category("残局").unwrap();let c=lib.snapshot().unwrap().categories[0].id.clone();lib.set_categories(&[id.clone()],&[c.clone()]).unwrap();lib.delete_category(&c).unwrap();assert!(lib.recording(&id).unwrap().category_ids.is_empty());}
    #[test] fn does_not_forget_permanent_copy(){let (_dir,mut lib,path)=fixture();lib.import_files(&[path.to_string_lossy().into()],None).unwrap();let id=lib.snapshot().unwrap().recordings[0].id.clone();lib.favorites(&[id.clone()],"copy").unwrap();assert!(lib.delete_recordings(&[id.clone()],"index").is_err());assert!(lib.recording(&id).is_ok());}
    #[test] fn app_data_is_excluded_even_when_scanning_parent(){let (dir,mut lib,path)=fixture();let copy=lib.data_dir.join("favorites/kept.mp4");fs::write(copy,b"saved video").unwrap();assert!(lib.add_source(lib.data_dir.to_str().unwrap(),"invalid","import").is_err());lib.add_source(dir.path().to_str().unwrap(),"parent","import").unwrap();lib.scan().unwrap();let rows=lib.snapshot().unwrap().recordings;assert_eq!(rows.len(),1);assert_eq!(path_key(Path::new(&rows[0].path)),path_key(&path));}
    #[cfg(windows)]
    #[test]
    #[ignore="Uses Windows Recycle Bin on generated disposable fixtures"]
    fn recycle_bin_smoke(){
        let (_dir,mut lib,path)=fixture();lib.import_files(&[path.to_string_lossy().into()],None).unwrap();let id=lib.snapshot().unwrap().recordings[0].id.clone();lib.favorites(&[id.clone()],"copy").unwrap();let copy=lib.recording(&id).unwrap().favorite_path.unwrap();
        lib.delete_recordings(&[id.clone()],"original").unwrap();assert!(!path.exists());assert!(Path::new(&copy).is_file());assert!(!lib.snapshot().unwrap().recordings[0].available);assert!(lib.playback_path(&lib.recording(&id).unwrap()).is_ok());
        lib.delete_recordings(&[id.clone()],"copy").unwrap();assert!(!Path::new(&copy).exists());assert!(lib.recording(&id).unwrap().favorite_path.is_none());
    }
}
