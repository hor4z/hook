use crate::Image;
use crate::store::{Store, now_ms};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub created: u64,
    pub size: [u32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

pub fn clean_path(path: &str) -> String {
    path.split('/').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/")
}

pub fn split_path(name: &str) -> (String, String) {
    match name.rsplit_once('/') {
        Some((dir, leaf)) if !leaf.trim().is_empty() => (clean_path(dir), leaf.trim().to_string()),
        _ => (String::new(), name.trim().to_string()),
    }
}

pub fn palette(img: &Image, max: usize) -> Vec<String> {
    let mut bins: std::collections::HashMap<u16, (u64, [u64; 3])> = std::collections::HashMap::new();
    let step = ((img.w as u64 * img.h as u64) / 40_000).max(1) as usize;
    for px in img.rgba.chunks_exact(4).step_by(step) {
        if px[3] < 128 {
            continue;
        }
        let key = ((px[0] as u16 >> 4) << 8) | ((px[1] as u16 >> 4) << 4) | (px[2] as u16 >> 4);
        let e = bins.entry(key).or_insert((0, [0; 3]));
        e.0 += 1;
        for c in 0..3 {
            e.1[c] += px[c] as u64;
        }
    }
    let total: u64 = bins.values().map(|b| b.0).sum();
    let mut all: Vec<(u64, [u8; 3])> = bins.values().map(|(n, s)| (*n, [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8])).collect();
    all.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<(u64, [u8; 3])> = Vec::new();
    for (n, c) in all {
        if n * 100 < total {
            break;
        }
        match out.iter_mut().find(|(_, o)| o.iter().zip(c).map(|(a, b)| (*a as i32 - b as i32).abs()).sum::<i32>() < 36) {
            Some(o) => o.0 += n,
            None => out.push((n, c)),
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter()
        .take(max)
        .map(|(n, c)| format!("#{:02x}{:02x}{:02x} {}%", c[0], c[1], c[2], (n * 100 + total / 2) / total.max(1)))
        .collect()
}

pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').chars().take(48).collect::<String>();
    if out.is_empty() { "memory".into() } else { out }
}

impl Store {
    pub fn memories_dir(&self) -> PathBuf {
        self.root.join("memories")
    }

    pub fn memory_file(&self, id: &str, name: &str) -> PathBuf {
        self.memories_dir().join(id).join(name)
    }

    pub fn save_memory(&self, name: &str, note: &str, image: &Image, monitor: Option<String>) -> io::Result<Memory> {
        let (path, name) = split_path(name);
        let name = name.as_str();
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.memories_dir().join(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let dir = self.memories_dir().join(&id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("image.png"), image.encode_png()?)?;
        std::fs::write(dir.join("thumb.png"), image.fit(360).encode_png()?)?;
        let m = Memory {
            id,
            name: name.trim().to_string(),
            note: note.trim().to_string(),
            created: now_ms(),
            size: [image.w, image.h],
            monitor,
            palette: palette(image, 6),
            path,
        };
        let tmp = dir.join("memory.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&m).map_err(io::Error::other)?)?;
        std::fs::rename(tmp, dir.join("memory.json"))?;
        Ok(m)
    }

    pub fn memories(&self) -> Vec<Memory> {
        let Ok(entries) = std::fs::read_dir(self.memories_dir()) else { return Vec::new() };
        let mut out: Vec<Memory> = entries
            .flatten()
            .filter_map(|e| std::fs::read(e.path().join("memory.json")).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
        out
    }

    pub fn memory(&self, key: &str) -> Option<Memory> {
        let all = self.memories();
        let k = key.trim().trim_start_matches('@');
        let s = slug(k);
        all.iter()
            .find(|m| m.id == k)
            .or_else(|| all.iter().find(|m| m.id == s))
            .or_else(|| all.iter().find(|m| m.name.eq_ignore_ascii_case(k)))
            .cloned()
    }

    pub fn mentioned(&self, text: &str) -> Vec<Memory> {
        let all = self.memories();
        let mut out: Vec<Memory> = Vec::new();
        for word in text.split(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | '(' | ')')) {
            let Some(k) = word.strip_prefix('@') else { continue };
            if let Some(m) = all.iter().find(|m| m.id == k || m.id == slug(k))
                && !out.iter().any(|x| x.id == m.id)
            {
                out.push(m.clone());
            }
        }
        out
    }

    pub fn move_memory(&self, id: &str, path: &str) -> io::Result<Memory> {
        if id.is_empty() || id.contains('/') || id.contains("..") {
            return Err(io::Error::other("bad memory id"));
        }
        let file = self.memories_dir().join(id).join("memory.json");
        let mut m: Memory = serde_json::from_slice(&std::fs::read(&file)?).map_err(io::Error::other)?;
        m.path = clean_path(path);
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&m).map_err(io::Error::other)?)?;
        std::fs::rename(tmp, &file)?;
        Ok(m)
    }

    pub fn delete_memory(&self, id: &str) -> io::Result<()> {
        if id.is_empty() || id.contains('/') || id.contains("..") {
            return Err(io::Error::other("bad memory id"));
        }
        std::fs::remove_dir_all(self.memories_dir().join(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> Store {
        let root = std::env::temp_dir().join(format!("koon-memory-{name}-{}", now_ms()));
        Store::at(root)
    }

    #[test]
    fn palette_finds_the_dominant_colours() {
        let mut img = Image::new(100, 100);
        for (i, px) in img.rgba.chunks_exact_mut(4).enumerate() {
            let c = if i % 100 < 70 { [255, 255, 255] } else { [59, 111, 224] };
            px.copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
        assert_eq!(palette(&img, 6), ["#ffffff 70%", "#3b6fe0 30%"]);
    }

    #[test]
    fn slug_is_ascii_and_readable() {
        assert_eq!(slug("  Nav de Vercel!! "), "nav-de-vercel");
        assert_eq!(slug("Botón rosa · hover"), "boton-rosa-hover");
        assert_eq!(slug("???"), "memory");
    }

    #[test]
    fn save_list_find_and_mentions() {
        let store = temp("save");
        let img = Image::new(40, 20);
        let a = store.save_memory("Tarjeta Linear", "me gusta el borde", &img, None).unwrap();
        let b = store.save_memory("Tarjeta Linear", "", &img, None).unwrap();
        assert_eq!(a.id, "tarjeta-linear");
        assert_eq!(b.id, "tarjeta-linear-2");
        assert_eq!(store.memories().len(), 2);
        assert!(store.memory_file(&a.id, "thumb.png").exists());
        assert_eq!(store.memory("@tarjeta-linear").map(|m| m.note), Some("me gusta el borde".into()));
        assert_eq!(store.memory("Tarjeta Linear").map(|m| m.id), Some("tarjeta-linear".into()));
        let found = store.mentioned("hacelo como @tarjeta-linear-2, y también @tarjeta-linear.");
        assert_eq!(found.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["tarjeta-linear-2", "tarjeta-linear"]);
        let c = store.save_memory(" Botones / Web /Primario ", "", &img, None).unwrap();
        assert_eq!((c.id.as_str(), c.name.as_str(), c.path.as_str()), ("primario", "Primario", "Botones/Web"));
        assert_eq!(store.move_memory(&c.id, "/Otros//").unwrap().path, "Otros");
        assert_eq!(store.memory("primario").map(|m| m.path), Some("Otros".into()));
        store.delete_memory(&c.id).unwrap();
        store.delete_memory(&a.id).unwrap();
        assert_eq!(store.memories().len(), 1);
        assert!(store.delete_memory("../x").is_err());
        let _ = std::fs::remove_dir_all(&store.root);
    }
}
