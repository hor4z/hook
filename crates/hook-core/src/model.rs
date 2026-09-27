use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pin,
    Stroke,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Draft,
    Pending,
    Taken,
    Resolved,
}

impl Status {
    pub fn open(self) -> bool {
        matches!(self, Status::Pending | Status::Taken)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    User,
    Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Say {
    Reply,
    Question,
    Done,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub author: Author,
    pub say: Say,
    pub text: String,
    pub at: u64,
}

impl Entry {
    pub fn new(author: Author, say: Say, text: impl Into<String>) -> Entry {
        Entry {
            author,
            say,
            text: text.into(),
            at: crate::store::now_ms(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub id: u32,
    pub kind: Kind,
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thread: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

impl Mark {
    pub fn new(id: u32, kind: Kind, at: [f32; 2]) -> Mark {
        Mark {
            id,
            kind,
            points: vec![at],
            text: String::new(),
            status: Status::Draft,
            audio: None,
            note: None,
            thread: Vec::new(),
            memory: None,
        }
    }

    pub fn last(&self) -> Option<&Entry> {
        self.thread.last()
    }

    pub fn asking(&self) -> Option<&str> {
        self.last().filter(|e| e.author == Author::Agent && e.say == Say::Question).map(|e| e.text.as_str())
    }

    pub fn done(&self) -> Option<&str> {
        self.last().filter(|e| e.author == Author::Agent && e.say == Say::Done).map(|e| e.text.as_str())
    }

    pub fn shift(&mut self, dx: f32, dy: f32) {
        for p in &mut self.points {
            p[0] += dx;
            p[1] += dy;
        }
    }

    pub fn merge_thread(&mut self, other: &[Entry]) {
        for e in other {
            if !self.thread.iter().any(|x| x.at == e.at && x.author == e.author && x.text == e.text) {
                self.thread.push(e.clone());
            }
        }
        self.thread.sort_by_key(|e| e.at);
    }

    pub fn bounds(&self) -> [f32; 4] {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &self.points {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        if self.points.is_empty() {
            return [0.0; 4];
        }
        [x0, y0, x1 - x0, y1 - y0]
    }

    pub fn anchor(&self) -> [f32; 2] {
        match self.kind {
            Kind::Pin => self.points[0],
            Kind::Stroke => *self.points.last().unwrap_or(&[0.0, 0.0]),
            Kind::Area => {
                let b = self.bounds();
                [b[0] + b[2], b[1]]
            }
        }
    }

    pub fn meaningful(&self) -> bool {
        match self.kind {
            Kind::Pin => !self.text.trim().is_empty() || self.audio.is_some(),
            Kind::Stroke => self.points.len() > 1,
            Kind::Area => {
                let b = self.bounds();
                b[2] >= 4.0 && b[3] >= 4.0
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created: u64,
    pub size: [f32; 2],
    pub scale: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub monitors: Vec<(String, [f32; 4])>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default)]
    pub marks: Vec<Mark>,
}

impl Session {
    pub fn next_id(&self) -> u32 {
        self.marks.iter().map(|m| m.id).max().unwrap_or(0) + 1
    }

    pub fn mark(&self, id: u32) -> Option<&Mark> {
        self.marks.iter().find(|m| m.id == id)
    }

    pub fn mark_mut(&mut self, id: u32) -> Option<&mut Mark> {
        self.marks.iter_mut().find(|m| m.id == id)
    }

    pub fn open(&self) -> impl Iterator<Item = &Mark> {
        self.marks.iter().filter(|m| m.status.open())
    }

    pub fn monitor_at(&self, p: [f32; 2]) -> Option<&str> {
        self.monitors
            .iter()
            .find(|(_, r)| p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3])
            .map(|(n, _)| n.as_str())
            .or(self.monitor.as_deref())
    }

    pub fn to_physical(&self, p: [f32; 2]) -> [f32; 2] {
        [p[0] * self.scale, p[1] * self.scale]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_cover_every_point() {
        let mut m = Mark::new(1, Kind::Stroke, [10.0, 20.0]);
        m.points.extend([[4.0, 30.0], [18.0, 25.0]]);
        assert_eq!(m.bounds(), [4.0, 20.0, 14.0, 10.0]);
        assert_eq!(m.anchor(), [18.0, 25.0]);
    }

    #[test]
    fn empty_pins_and_tiny_areas_are_not_meaningful() {
        let pin = Mark::new(1, Kind::Pin, [0.0, 0.0]);
        assert!(!pin.meaningful());
        let mut area = Mark::new(2, Kind::Area, [0.0, 0.0]);
        area.points.push([2.0, 2.0]);
        assert!(!area.meaningful());
        area.points[1] = [40.0, 30.0];
        assert!(area.meaningful());
        assert_eq!(area.anchor(), [40.0, 0.0]);
    }

    #[test]
    fn threads_merge_without_duplicates_in_time_order() {
        let mut m = Mark::new(1, Kind::Pin, [0.0, 0.0]);
        let q = Entry {
            author: Author::Agent,
            say: Say::Question,
            text: "green or gold?".into(),
            at: 10,
        };
        let a = Entry {
            author: Author::User,
            say: Say::Reply,
            text: "gold".into(),
            at: 20,
        };
        m.thread.push(a.clone());
        m.merge_thread(&[q.clone(), a.clone()]);
        assert_eq!(m.thread, vec![q, a]);
        assert_eq!(m.asking(), None);
        m.thread.push(Entry {
            author: Author::Agent,
            say: Say::Done,
            text: "done".into(),
            at: 30,
        });
        assert_eq!(m.done(), Some("done"));
    }

    #[test]
    fn statuses_serialize_in_snake_case() {
        assert_eq!(serde_json::to_string(&Status::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::from_str::<Kind>("\"area\"").unwrap(), Kind::Area);
        assert!(Status::Taken.open() && !Status::Draft.open() && !Status::Resolved.open());
    }

    #[test]
    fn physical_coordinates_follow_the_scale() {
        let s = Session {
            id: "x".into(),
            created: 0,
            size: [100.0, 50.0],
            scale: 2.0,
            monitor: None,
            monitors: vec![("DP-2".into(), [0.0, 0.0, 50.0, 50.0]), ("HDMI-1".into(), [50.0, 0.0, 50.0, 50.0])],
            screen: None,
            annotated: None,
            preview: None,
            marks: vec![],
        };
        assert_eq!(s.to_physical([3.0, 4.5]), [6.0, 9.0]);
        assert_eq!(s.monitor_at([60.0, 10.0]), Some("HDMI-1"));
        assert_eq!(s.monitor_at([10.0, 10.0]), Some("DP-2"));
        assert_eq!(s.next_id(), 1);
    }
}
