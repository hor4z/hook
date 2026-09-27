use crate::model::Session;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Store {
    pub root: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Prefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pill: Option<(i32, i32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_server: Option<String>,
}

fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = env("HOME").or_else(|| env("USERPROFILE")).unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else if cfg!(windows) {
        env("APPDATA").unwrap_or(home)
    } else {
        env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Store {
    pub fn open() -> Store {
        let root = std::env::var_os("KOON_HOME").map(PathBuf::from).unwrap_or_else(|| data_dir().join("koon"));
        Store::at(root)
    }

    pub fn at(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn sessions(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn dir(&self, id: &str) -> PathBuf {
        self.sessions().join(id)
    }

    pub fn file(&self, id: &str, name: &str) -> PathBuf {
        self.dir(id).join(name)
    }

    pub fn create(&self, size: [f32; 2], scale: f32) -> io::Result<Session> {
        let created = now_ms();
        let mut id = created.to_string();
        let mut n = 1;
        while self.dir(&id).exists() {
            id = format!("{created}-{n}");
            n += 1;
        }
        std::fs::create_dir_all(self.dir(&id))?;
        Ok(Session {
            id,
            created,
            size,
            scale,
            monitor: None,
            monitors: Vec::new(),
            screen: None,
            annotated: None,
            preview: None,
            marks: Vec::new(),
        })
    }

    pub fn save(&self, s: &Session) -> io::Result<()> {
        let dir = self.dir(&s.id);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("session.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(s).map_err(io::Error::other)?)?;
        std::fs::rename(tmp, dir.join("session.json"))
    }

    pub fn write(&self, id: &str, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let path = self.file(id, name);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }

    pub fn load(&self, id: &str) -> io::Result<Session> {
        let bytes = std::fs::read(self.file(id, "session.json"))?;
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }

    pub fn modified(&self, id: &str) -> Option<SystemTime> {
        std::fs::metadata(self.file(id, "session.json")).and_then(|m| m.modified()).ok()
    }

    pub fn list(&self) -> Vec<Session> {
        let Ok(entries) = std::fs::read_dir(self.sessions()) else { return Vec::new() };
        let mut out: Vec<Session> = entries.flatten().filter_map(|e| e.file_name().to_str().and_then(|id| self.load(id).ok())).collect();
        out.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        out
    }

    pub fn update(&self, id: &str, f: impl FnOnce(&mut Session)) -> io::Result<Session> {
        let mut s = self.load(id)?;
        f(&mut s);
        self.save(&s)?;
        Ok(s)
    }

    pub fn prefs(&self) -> Prefs {
        std::fs::read(self.root.join("prefs.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save_prefs(&self, prefs: &Prefs) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let tmp = self.root.join("prefs.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(prefs).map_err(io::Error::other)?)?;
        std::fs::rename(tmp, self.root.join("prefs.json"))
    }

    pub fn path_of(&self, id: &str, name: Option<&str>) -> Option<PathBuf> {
        name.map(|n| self.file(id, n)).filter(|p| Path::exists(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Mark, Status};

    fn temp(tag: &str) -> Store {
        let root = std::env::temp_dir().join(format!("koon-test-{tag}-{}-{}", std::process::id(), now_ms()));
        let _ = std::fs::remove_dir_all(&root);
        Store::at(root)
    }

    #[test]
    fn sessions_round_trip_and_list_in_creation_order() {
        let store = temp("list");
        let mut a = store.create([800.0, 600.0], 1.0).unwrap();
        let mut m = Mark::new(1, Kind::Pin, [10.0, 10.0]);
        m.text = "bigger — más grande".into();
        m.status = Status::Pending;
        a.marks.push(m);
        store.save(&a).unwrap();
        let b = store.create([800.0, 600.0], 2.0).unwrap();
        store.save(&b).unwrap();
        assert_ne!(a.id, b.id);
        let all = store.list();
        assert_eq!(all.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), vec![a.id.clone(), b.id.clone()]);
        assert_eq!(all[0].marks[0].text, "bigger — más grande");
        let _ = std::fs::remove_dir_all(&store.root);
    }

    #[test]
    fn prefs_round_trip_and_default_when_missing() {
        let store = temp("prefs");
        assert_eq!(store.prefs(), Prefs::default());
        store
            .save_prefs(&Prefs {
                pill: Some((120, -40)),
                ..Prefs::default()
            })
            .unwrap();
        assert_eq!(store.prefs().pill, Some((120, -40)));
        let _ = std::fs::remove_dir_all(&store.root);
    }

    #[test]
    fn update_persists_status_changes() {
        let store = temp("update");
        let mut s = store.create([10.0, 10.0], 1.0).unwrap();
        s.marks.push(Mark::new(1, Kind::Pin, [1.0, 1.0]));
        store.save(&s).unwrap();
        store.update(&s.id, |s| s.marks[0].status = Status::Resolved).unwrap();
        assert_eq!(store.load(&s.id).unwrap().marks[0].status, Status::Resolved);
        assert!(store.modified(&s.id).is_some());
        let _ = std::fs::remove_dir_all(&store.root);
    }
}
