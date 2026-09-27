pub mod atlas;
pub mod bitmap;
pub mod cff;
pub mod fallback;
pub mod fonts;
pub mod gamma;
mod icon_table {
    include!(concat!(env!("OUT_DIR"), "/icon_table.rs"));
}
pub mod icons;
pub mod metrics;
pub mod raster;
pub mod shape;

pub use fonts::Face;

pub fn shaping_face(face: Face) -> Face {
    match face {
        Face::Mono500 => Face::Mono400,
        f => f,
    }
}

pub type Rgba = [f32; 4];

pub fn icon_source(name: &str) -> Option<&'static str> {
    icon_table::ICONS.binary_search_by(|(n, _)| (*n).cmp(name)).ok().map(|i| icon_table::ICONS[i].1)
}

pub fn icon_names() -> impl Iterator<Item = &'static str> {
    icon_table::ICONS.iter().map(|(n, _)| *n)
}
