use std::fmt::Write;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".svg").map(String::from))
        .collect();
    names.sort();
    let mut out = String::from("pub const ICONS: &[(&str, &str)] = &[\n");
    for n in &names {
        writeln!(out, "    ({n:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/icons/{n}.svg\"))),").unwrap();
    }
    out.push_str("];\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icon_table.rs"), out).unwrap();
}
