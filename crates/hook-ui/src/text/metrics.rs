use crate::text::fallback::{embedded, measure_char_with};
use crate::text::{Face, atlas, shape, shaping_face};
use std::collections::{BTreeSet, HashMap};

pub struct TextMetrics {
    system_fonts: bool,
    widths: HashMap<(Face, u32, char), f32>,
    shaped: HashMap<(String, Face, u32, u32), f32>,
    missing: BTreeSet<char>,
}

impl Default for TextMetrics {
    fn default() -> Self {
        TextMetrics::new()
    }
}

impl TextMetrics {
    pub fn new() -> TextMetrics {
        TextMetrics {
            system_fonts: true,
            widths: HashMap::new(),
            shaped: HashMap::new(),
            missing: BTreeSet::new(),
        }
    }

    pub fn strict() -> TextMetrics {
        TextMetrics {
            system_fonts: false,
            ..TextMetrics::new()
        }
    }

    pub fn is_strict(&self) -> bool {
        !self.system_fonts
    }

    pub fn measure(&mut self, text: &str, face: Face, size: f32, tracking: f32) -> f32 {
        let key = (text.to_string(), face, size.to_bits(), tracking.to_bits());
        if let Some(w) = self.shaped.get(&key) {
            return *w;
        }
        self.note_missing(text, face);
        let w = shape::measure(shaping_face(face), text, size, if size > 0.0 { tracking / size } else { 0.0 });
        if self.shaped.len() >= 20_000 {
            self.shaped.clear();
        }
        self.shaped.insert(key, w);
        w
    }

    pub fn metrics(&self, face: Face) -> (f32, f32) {
        shape::metrics(shaping_face(face))
    }

    pub fn canvas_advance(&mut self, face: Face, ch: char, px: f32) -> f32 {
        let key = (face, (px * 100.0) as u32 + 7_000_000, ch);
        if let Some(w) = self.widths.get(&key) {
            return *w;
        }
        let (_, id, m) = atlas::measure(shaping_face(face), ch, px);
        let w = if id.is_some() {
            m.advance as f32
        } else {
            measure_char_with(face, ch, px, self.system_fonts).advance
        };
        self.widths.insert(key, w);
        w
    }

    pub fn measure_canvas(&mut self, text: &str, face: Face, size: f32, scale: f32) -> f32 {
        self.note_missing(text, face);
        let px = (size * scale * 4.0).round() / 4.0;
        text.chars().map(|ch| self.canvas_advance(face, ch, px)).sum::<f32>() / scale
    }

    pub fn missing(&self) -> impl Iterator<Item = char> + '_ {
        self.missing.iter().copied()
    }

    fn note_missing(&mut self, text: &str, face: Face) {
        if self.system_fonts {
            return;
        }
        let font = embedded(face);
        for ch in text.chars().filter(|c| !c.is_whitespace() && !c.is_control()) {
            if font.glyph(ch) == 0 && crate::text::fonts::resolve(shaping_face(face), ch).1.is_none() {
                self.missing.insert(ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_matches_shaping_with_the_mono_medium_rule() {
        let mut m = TextMetrics::new();
        assert_eq!(m.measure("Guardar", Face::Sans500, 13.0, 0.0), shape::measure(Face::Sans500, "Guardar", 13.0, 0.0));
        assert_eq!(m.measure("0123", Face::Mono500, 12.0, 0.0), shape::measure(Face::Mono400, "0123", 12.0, 0.0));
        assert_eq!(m.metrics(Face::Mono500), shape::metrics(Face::Mono400));
    }

    #[test]
    fn canvas_advances_are_cached_and_scaled_back_to_logical_pixels() {
        let mut m = TextMetrics::new();
        let one = m.measure_canvas("Kibo", Face::Sans400, 13.0, 1.0);
        assert_eq!(m.measure_canvas("Kibo", Face::Sans400, 13.0, 1.0), one);
        let two = m.measure_canvas("Kibo", Face::Sans400, 13.0, 2.0);
        assert!((two - one).abs() < 1.5, "{one} {two}");
    }

    #[test]
    fn strict_mode_reports_missing_characters_and_never_uses_system_fonts() {
        let mut m = TextMetrics::strict();
        m.measure("mover ⇄ aquí", Face::Sans400, 13.0, 0.0);
        assert_eq!(m.missing().collect::<Vec<_>>(), vec!['⇄']);
        assert!(m.is_strict());
        let mut loose = TextMetrics::new();
        loose.measure("⇄", Face::Sans400, 13.0, 0.0);
        assert_eq!(loose.missing().count(), 0);
    }

    #[test]
    fn mono_medium_shapes_as_mono_regular() {
        assert_eq!(shaping_face(Face::Mono500), Face::Mono400);
        assert_eq!(shaping_face(Face::Sans600), Face::Sans600);
    }
}
