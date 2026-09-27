use crate::core::{Cursor, Describe, Face, Field, Key, Mods, R, Rgba, Role, Ui, ValueRef, alpha};

pub struct FieldResponse {
    pub changed: bool,
    pub focused: bool,
    pub submitted: Option<Mods>,
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i)
}

fn word_left(s: &[char], mut i: usize) -> usize {
    while i > 0 && s[i - 1] == ' ' {
        i -= 1;
    }
    while i > 0 && s[i - 1] != ' ' {
        i -= 1;
    }
    i
}

fn word_right(s: &[char], mut i: usize) -> usize {
    while i < s.len() && s[i] == ' ' {
        i += 1;
    }
    while i < s.len() && s[i] != ' ' {
        i += 1;
    }
    i
}

pub fn text_field(ui: &mut Ui, key: u64, r: R, value: &mut String, placeholder: &str, face: Face, size: f32, color: Rgba) -> FieldResponse {
    field(ui, key, r, value, placeholder, face, size, color, false)
}

fn caret_at(ui: &mut Ui, value: &str, face: Face, size: f32, x: f32) -> usize {
    let mut prev = 0.0;
    for (i, (b, ch)) in value.char_indices().enumerate() {
        let next = ui.measure(&value[..b + ch.len_utf8()], face, size);
        if x < (prev + next) / 2.0 {
            return i;
        }
        prev = next;
    }
    value.chars().count()
}

fn word_bounds(chars: &[char], at: usize) -> (usize, usize) {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let at = at.min(chars.len());
    let kind = chars.get(at).or_else(|| at.checked_sub(1).and_then(|i| chars.get(i))).map(|c| is_word(*c));
    let Some(kind) = kind else { return (at, at) };
    let mut a = at;
    while a > 0 && is_word(chars[a - 1]) == kind && !chars[a - 1].is_whitespace() {
        a -= 1;
    }
    let mut b = at;
    while b < chars.len() && is_word(chars[b]) == kind && !chars[b].is_whitespace() {
        b += 1;
    }
    (a, b)
}

#[allow(clippy::too_many_arguments)]
fn field(ui: &mut Ui, key: u64, r: R, value: &mut String, placeholder: &str, face: Face, size: f32, color: Rgba, secret: bool) -> FieldResponse {
    let css = ui.css.clone();
    ui.region(r, key);
    if ui.describing() {
        let shown = if secret { "•".repeat(value.chars().count()) } else { value.clone() };
        ui.describe(Describe::new(key, Role::TextInput, r).name(placeholder).value(ValueRef::Text(&shown)));
    }
    if ui.hovered(key) {
        ui.cursor(Cursor::Text);
    }
    let mut f = ui.s.fields.remove(&key).unwrap_or_default();
    let chars: Vec<char> = value.chars().collect();
    f.caret = f.caret.min(chars.len());
    f.anchor = f.anchor.min(chars.len());
    let local = ui.input.mouse.0 - (ui.map(r).x - f.scroll);
    let shown_of = |v: &str| if secret { "•".repeat(v.chars().count()) } else { v.to_string() };
    let before = shown_of(value);
    if ui.pressed_on(key) {
        ui.s.focus = Some(key);
        ui.s.active = Some(key);
        let at = caret_at(ui, &before, face, size, local);
        if ui.input.double {
            let (a, b) = if secret { (0, chars.len()) } else { word_bounds(&chars, at) };
            f.anchor = a;
            f.caret = b;
        } else {
            f.caret = at;
            if !ui.input.mods.shift {
                f.anchor = at;
            }
        }
    } else if ui.s.active == Some(key) && ui.input.down {
        f.caret = caret_at(ui, &before, face, size, local);
        ui.s.animating = true;
    }
    let focused = ui.s.focus == Some(key);
    let mut changed = false;
    let mut submitted = None;
    if focused {
        let mut chars = chars;
        let remove_sel = |chars: &mut Vec<char>, f: &mut Field| -> bool {
            if f.caret == f.anchor {
                return false;
            }
            let (a, b) = (f.caret.min(f.anchor), f.caret.max(f.anchor));
            chars.drain(a..b);
            f.caret = a;
            f.anchor = a;
            true
        };
        let mut text = ui.input.text.clone();
        if let Some(p) = &ui.input.paste {
            text.push_str(&p.replace(['\n', '\r'], " "));
        }
        let typed: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
        if !typed.is_empty() {
            remove_sel(&mut chars, &mut f);
            for c in typed {
                chars.insert(f.caret, c);
                f.caret += 1;
            }
            f.anchor = f.caret;
            changed = true;
        }
        for (k, m) in ui.input.keys.clone() {
            let word = m.ctrl || m.alt;
            match k {
                Key::Backspace => {
                    if !remove_sel(&mut chars, &mut f) && f.caret > 0 {
                        let to = if word { word_left(&chars, f.caret) } else { f.caret - 1 };
                        chars.drain(to..f.caret);
                        f.caret = to;
                        f.anchor = to;
                    }
                    changed = true;
                }
                Key::Delete => {
                    if !remove_sel(&mut chars, &mut f) && f.caret < chars.len() {
                        let to = if word { word_right(&chars, f.caret) } else { f.caret + 1 };
                        chars.drain(f.caret..to);
                    }
                    changed = true;
                }
                Key::Left => {
                    f.caret = if word {
                        word_left(&chars, f.caret)
                    } else if f.caret != f.anchor && !m.shift {
                        f.caret.min(f.anchor)
                    } else {
                        f.caret.saturating_sub(1)
                    };
                    if !m.shift {
                        f.anchor = f.caret;
                    }
                }
                Key::Right => {
                    f.caret = if word {
                        word_right(&chars, f.caret)
                    } else if f.caret != f.anchor && !m.shift {
                        f.caret.max(f.anchor)
                    } else {
                        (f.caret + 1).min(chars.len())
                    };
                    if !m.shift {
                        f.anchor = f.caret;
                    }
                }
                Key::Home => {
                    f.caret = 0;
                    if !m.shift {
                        f.anchor = 0;
                    }
                }
                Key::End => {
                    f.caret = chars.len();
                    if !m.shift {
                        f.anchor = f.caret;
                    }
                }
                Key::Char('a') if m.ctrl => {
                    f.anchor = 0;
                    f.caret = chars.len();
                }
                Key::Char('c' | 'x') if m.ctrl && f.caret != f.anchor && !secret => {
                    let (a, b) = (f.caret.min(f.anchor), f.caret.max(f.anchor));
                    ui.s.copy_out = Some(chars[a..b].iter().collect());
                    if k == Key::Char('x') {
                        remove_sel(&mut chars, &mut f);
                        changed = true;
                    }
                }
                Key::Enter => submitted = Some(m),
                _ => {}
            }
        }
        if changed {
            *value = chars.iter().collect();
        }
    }
    let shown = shown_of(value);
    let value = &shown;
    let chars: Vec<char> = value.chars().collect();
    let cy = r.y + r.h / 2.0;
    let base = ui.baseline(face, size, cy);
    let total = ui.measure(value, face, size);
    let caret_rel = ui.measure(&value[..byte_at(value, f.caret)], face, size);
    if focused {
        if caret_rel - f.scroll > r.w - 2.0 {
            f.scroll = caret_rel - r.w + 2.0;
        } else if caret_rel < f.scroll {
            f.scroll = caret_rel;
        }
    }
    f.scroll = f.scroll.clamp(0.0, (total - r.w + 2.0).max(0.0));
    let x0 = r.x - f.scroll;
    ui.clip(r);
    if value.is_empty() {
        ui.text(r.x, base, placeholder, face, size, css.faint);
    }
    let caret_x = x0 + caret_rel;
    if focused && f.caret != f.anchor {
        let (a, b) = (f.caret.min(f.anchor), f.caret.max(f.anchor));
        let xa = x0 + ui.measure(&value[..byte_at(value, a)], face, size);
        let xb = x0 + ui.measure(&value[..byte_at(value, b)], face, size);
        ui.rect(R::new(xa, cy - size * 0.7, xb - xa, size * 1.4), 0.0, alpha(css.info, 0.3));
    }
    ui.text(x0, base, value, face, size, color);
    if focused {
        let blink = ((ui.now / 530.0) as i64) % 2 == 0;
        ui.s.animating = true;
        if blink || changed {
            ui.rect(R::new(caret_x.round(), cy - size * 0.62, 1.0, size * 1.24), 0.0, css.ink);
        }
    }
    ui.unclip();
    let _ = chars;
    ui.s.fields.insert(key, f);
    FieldResponse { changed, focused, submitted }
}
