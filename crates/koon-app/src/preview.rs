use crate::gfx::Gpu;
use crate::look::{self, Act, DockLook, PillLook};
use koon_core::{Image, Kind, Mark, Status};
use koon_ui::{Css, R, hex, id};

fn scene() -> Vec<Mark> {
    let mut pin = Mark::new(1, Kind::Pin, [300.0, 210.0]);
    pin.text = "Este botón debería ser verde y más grande".into();
    pin.status = Status::Pending;
    let mut stroke = Mark::new(2, Kind::Stroke, [680.0, 380.0]);
    for i in 1..64 {
        let t = i as f32 / 60.0 * std::f32::consts::TAU;
        stroke.points.push([600.0 + 80.0 * t.cos(), 380.0 + 40.0 * t.sin()]);
    }
    stroke.text = "Sacar este bloque".into();
    let mut area = Mark::new(3, Kind::Area, [170.0, 470.0]);
    area.points.push([470.0, 610.0]);
    area.status = Status::Taken;
    vec![pin, stroke, area]
}

pub fn render(path: &str) -> Result<(), String> {
    let gpu = Gpu::headless();
    let (w, h, scale) = (1100.0, 720.0, 2.0);
    let css = Css::light();
    let marks = scene();
    let mut text = String::from("Que el título use la fuente del sistema");
    let img = gpu.frames((w * scale) as u32, (h * scale) as u32, scale, &css, 1000.0, 12, |ui, _| {
        ui.rect(R::new(0.0, 0.0, w, h), 0.0, hex("#E9EDF2", 1.0));
        ui.rect(R::new(0.0, 0.0, w, 56.0), 0.0, hex("#FFFFFF", 1.0));
        ui.rect(R::new(140.0, 170.0, 340.0, 80.0), 12.0, hex("#FFFFFF", 1.0));
        ui.rect(R::new(160.0, 196.0, 120.0, 28.0), 8.0, hex("#3B6FE0", 1.0));
        ui.rect(R::new(520.0, 330.0, 170.0, 100.0), 12.0, hex("#D3DAE3", 1.0));
        ui.rect(R::new(180.0, 480.0, 280.0, 120.0), 12.0, hex("#FFFFFF", 1.0));
        ui.rect(R::new(200.0, 500.0, 160.0, 14.0), 7.0, hex("#D3DAE3", 1.0));
        ui.rect(R::new(200.0, 526.0, 220.0, 10.0), 5.0, hex("#E4E9EF", 1.0));
        for m in &marks {
            look::mark(ui, m, 1.0, false, true);
        }
        let mut editing = Mark::new(4, Kind::Pin, [560.0, 540.0]);
        editing.text = text.clone();
        look::mark(ui, &editing, 1.0, true, false);
        look::bubble(
            ui,
            id(&["preview"]),
            look::badge_rect(&editing),
            R::new(0.0, 0.0, w, h),
            &mut text,
            "¿Qué hay que cambiar acá?",
            look::Mic::Listening(0.7),
            look::mark_color(&editing),
        );
        look::dock(
            ui,
            &DockLook {
                at: (w - look::DOCK_W - 24.0, 90.0),
                up: false,
                area: R::new(0.0, 0.0, w, h),
                open: true,
                linger: None,
                active: Some(Act::Draw),
            },
        );
        ui.push_offset(20.0, h - 150.0);
        look::pill(
            ui,
            &PillLook {
                level: 0.8,
                listening: true,
                busy: false,
                hidden: false,
                open: 3,
                working: true,
                shadow: 1.0,
            },
        );
        ui.pop_offset();
    });
    let mut out = Image::new(img.w, img.h);
    out.rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
    out.over_premultiplied(&img);
    std::fs::write(path, out.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn morph(path: &str) -> Result<(), String> {
    let gpu = Gpu::headless();
    let css = Css::light();
    let (w, h, scale) = (90.0, 400.0, 2.0);
    let shots = [0usize, 1, 2, 3, 4, 6, 9];
    let mut strip = Image::new((w * scale) as u32 * (shots.len() as u32 + 1), (h * scale) as u32);
    unsafe { std::env::set_var("KOON_STEP", "40") };
    let paint = |img: &Image, col: u32, strip: &mut Image| {
        let mut bg = Image::new(img.w, img.h);
        for p in bg.rgba.chunks_exact_mut(4) {
            p.copy_from_slice(&[40, 44, 52, 255]);
        }
        bg.over_premultiplied(img);
        for y in 0..img.h {
            let src = &bg.rgba[(y * img.w * 4) as usize..((y + 1) * img.w * 4) as usize];
            let at = ((y * strip.w + col * img.w) * 4) as usize;
            strip.rgba[at..at + src.len()].copy_from_slice(src);
        }
    };
    let pill = gpu.frames((w * scale) as u32, (h * scale) as u32, scale, &css, 0.0, 1, |ui, _| {
        ui.push_offset(0.0, 8.0);
        look::pill(
            ui,
            &PillLook {
                level: 0.0,
                listening: false,
                busy: false,
                hidden: false,
                open: 0,
                working: false,
                shadow: 1.0,
            },
        );
        ui.pop_offset();
    });
    paint(&pill, 0, &mut strip);
    for (k, n) in shots.iter().enumerate() {
        let img = gpu.frames((w * scale) as u32, (h * scale) as u32, scale, &css, 0.0, n + 1, |ui, i| {
            look::dock(
                ui,
                &DockLook {
                    at: (look::MARGIN, look::MARGIN + 8.0),
                    up: false,
                    area: R::new(0.0, 0.0, w, h),
                    open: i > 0,
                    linger: None,
                    active: None,
                },
            );
        });
        paint(&img, k as u32 + 1, &mut strip);
    }
    std::fs::write(path, strip.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn palette(path: &str) -> Result<(), String> {
    let gpu = Gpu::headless();
    let (w, h, scale) = (1100.0, 720.0, 2.0);
    let css = Css::light();
    let swatch = |a: [u8; 3], b: [u8; 3]| {
        let mut img = Image::new(320, 180);
        for y in 0..180u32 {
            for x in 0..320u32 {
                let t = (x + y) as f32 / 500.0;
                let i = ((y * 320 + x) * 4) as usize;
                for c in 0..3 {
                    img.rgba[i + c] = (a[c] as f32 * (1.0 - t) + b[c] as f32 * t) as u8;
                }
                img.rgba[i + 3] = 255;
            }
        }
        img
    };
    let card = |id: &str, name: &str, note: &str, path: &str, a: [u8; 3], b: [u8; 3]| look::Card {
        id: id.into(),
        name: name.into(),
        note: note.into(),
        path: path.into(),
        full: None,
        thumb: Some(swatch(a, b)),
    };
    let cards = || {
        vec![
            card("nav-vercel", "Nav de Vercel", "el blur y el borde fino", "", [20, 20, 24], [70, 70, 90]),
            card("tarjeta-linear", "Tarjeta Linear", "", "tarjetas", [94, 106, 210], [30, 30, 60]),
            card("tarjeta-stripe", "Tarjeta Stripe", "", "tarjetas", [99, 91, 255], [160, 220, 255]),
            card("tarjeta-notion", "Tarjeta Notion", "", "tarjetas", [245, 245, 240], [200, 200, 190]),
            card("boton-rosa", "Botón rosa hover", "cómo rebota", "botones", [240, 120, 170], [255, 200, 220]),
            card("boton-verde", "Botón verde", "", "botones", [60, 200, 120], [20, 90, 60]),
            card("hero-arc", "Hero de Arc", "", "landings/hero", [255, 140, 60], [255, 90, 130]),
        ]
    };
    let mut browse = look::Palette::new(cards());
    browse.sel = 1;
    let mut inside = look::Palette::new(cards());
    inside.dir = "tarjetas".into();
    let mut ask = look::Palette::new(cards());
    let full = std::env::temp_dir().join("koon-palette-full.png");
    std::fs::write(&full, swatch([240, 120, 170], [40, 40, 60]).encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    ask.cards[0].full = Some(full);
    ask.asking = Some(0);
    ask.text = "quiero este header en mi landing".into();
    for (name, p) in [("browse", &mut browse), ("inside", &mut inside), ("ask", &mut ask)] {
        let img = gpu.frames((w * scale) as u32, (h * scale) as u32, scale, &css, 0.0, 14, |ui, _| {
            ui.rect(R::new(0.0, 0.0, w, h), 0.0, hex("#E9EDF2", 1.0));
            ui.rect(R::new(0.0, 0.0, w, 56.0), 0.0, hex("#FFFFFF", 1.0));
            let _ = look::palette(ui, R::new(0.0, 0.0, w, h), p);
            look::toast(ui, R::new(0.0, 0.0, w, h), "Memory guardada · @nav-vercel", 1000.0);
        });
        let mut out = Image::new(img.w, img.h);
        out.rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
        out.over_premultiplied(&img);
        std::fs::write(path.replace(".png", &format!("-{name}.png")), out.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    Ok(())
}
