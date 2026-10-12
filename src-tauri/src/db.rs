use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub struct Database { conn: Connection }
fn err(e: impl std::fmt::Display) -> String { format!("本地索引：{e}") }
impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS sources(id TEXT PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS recordings(id TEXT PRIMARY KEY, path_key TEXT NOT NULL UNIQUE, body TEXT NOT NULL, stamp TEXT NOT NULL DEFAULT '');
            CREATE TABLE IF NOT EXISTS categories(id TEXT PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS ignored_paths(path_key TEXT PRIMARY KEY);
            PRAGMA user_version=1;").map_err(err)?;
        Ok(Self { conn })
    }
    fn all<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>,String> {
        let mut q = self.conn.prepare(&format!("SELECT body FROM {table} ORDER BY rowid")).map_err(err)?;
        let rows = q.query_map([], |r| r.get::<_,String>(0)).map_err(err)?;
        rows.map(|row| serde_json::from_str(&row.map_err(err)?).map_err(err)).collect()
    }
    fn save<T: Serialize>(&self, table: &str, id: &str, body: &T) -> Result<(),String> {
        let body = serde_json::to_string(body).map_err(err)?;
        self.conn.execute(&format!("INSERT INTO {table}(id,body) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body"),params![id,body]).map_err(err)?;
        Ok(())
    }
    pub fn sources(&self)->Result<Vec<Source>,String> { self.all("sources") }
    pub fn categories(&self)->Result<Vec<Category>,String> { self.all("categories") }
    pub fn recordings(&self)->Result<Vec<Recording>,String> { self.all("recordings") }
    pub fn recording_by_key(&self,key:&str)->Result<Option<Recording>,String> {
        let body:Option<String>=self.conn.query_row("SELECT body FROM recordings WHERE path_key=?1",[key],|r|r.get(0)).optional().map_err(err)?;
        body.map(|b|serde_json::from_str(&b).map_err(err)).transpose()
    }
    pub fn recording_by_id(&self,id:&str)->Result<Recording,String> {
        let body:String=self.conn.query_row("SELECT body FROM recordings WHERE id=?1",[id],|r|r.get(0)).map_err(err)?;
        serde_json::from_str(&body).map_err(err)
    }
    pub fn save_source(&self,v:&Source)->Result<(),String> { self.save("sources",&v.id,v) }
    pub fn save_category(&self,v:&Category)->Result<(),String> { self.save("categories",&v.id,v) }
    pub fn remove_source(&self,id:&str)->Result<(),String> { self.conn.execute("DELETE FROM sources WHERE id=?1",[id]).map_err(err)?; Ok(()) }
    pub fn remove_category(&self,id:&str)->Result<(),String> { self.conn.execute("DELETE FROM categories WHERE id=?1",[id]).map_err(err)?; Ok(()) }
    pub fn save_recording(&self,v:&Recording,key:&str,stamp:&str)->Result<(),String> {
        let body=serde_json::to_string(v).map_err(err)?;
        self.conn.execute("INSERT INTO recordings(id,path_key,body,stamp) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET body=excluded.body,stamp=CASE WHEN excluded.stamp='' THEN recordings.stamp ELSE excluded.stamp END",params![v.id,key,body,stamp]).map_err(err)?;
        Ok(())
    }
    pub fn stamp(&self,id:&str)->Result<String,String> { self.conn.query_row("SELECT stamp FROM recordings WHERE id=?1",[id],|r|r.get(0)).map_err(err) }
    pub fn forget(&self,id:&str,key:&str)->Result<(),String> {
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        let result=(||{ self.conn.execute("INSERT OR IGNORE INTO ignored_paths(path_key) VALUES(?1)",[key]).map_err(err)?; self.conn.execute("DELETE FROM recordings WHERE id=?1",[id]).map_err(err)?; Ok(()) })();
        if result.is_ok() { self.conn.execute_batch("COMMIT").map_err(err)?; } else { let _=self.conn.execute_batch("ROLLBACK"); }
        result
    }
    pub fn is_ignored(&self,key:&str)->Result<bool,String> { self.conn.query_row("SELECT EXISTS(SELECT 1 FROM ignored_paths WHERE path_key=?1)",[key],|r|r.get(0)).map_err(err) }
    pub fn unignore(&self,key:&str)->Result<(),String> { self.conn.execute("DELETE FROM ignored_paths WHERE path_key=?1",[key]).map_err(err)?; Ok(()) }
    pub fn settings(&self)->Result<Settings,String> {
        let mut q=self.conn.prepare("SELECT body FROM settings WHERE id=1").map_err(err)?;
        let mut rows=q.query([]).map_err(err)?;
        match rows.next().map_err(err)? {Some(r)=>serde_json::from_str(&r.get::<_,String>(0).map_err(err)?).map_err(err),None=>Ok(Settings::default())}
    }
    pub fn save_settings(&self,v:&Settings)->Result<(),String> { let body=serde_json::to_string(v).map_err(err)?; self.conn.execute("INSERT INTO settings(id,body) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body",[body]).map_err(err)?; Ok(()) }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn settings_and_categories_persist() {
        let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("index.sqlite3");
        {let db=Database::open(&path).unwrap(); let s=Settings { ffmpeg_path:"D:\\中文\\ffmpeg.exe".into(), continuous_play:false, auto_scan:true }; db.save_settings(&s).unwrap(); db.save_category(&Category{id:"one".into(),name:"残局".into()}).unwrap();}
        let db=Database::open(&path).unwrap(); assert_eq!(db.settings().unwrap().ffmpeg_path,"D:\\中文\\ffmpeg.exe"); assert_eq!(db.categories().unwrap()[0].name,"残局");
    }
}
