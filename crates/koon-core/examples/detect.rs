use koon_core::{Image, detect};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let img = Image::decode_png(&std::fs::read(&args[0]).unwrap()).unwrap();
    let mut out = img.clone();
    let colors = [[255, 40, 40], [40, 200, 40], [40, 90, 255], [255, 160, 0], [200, 0, 200], [0, 200, 200]];
    for pair in args[2..].chunks(2) {
        let at = (pair[0].parse().unwrap(), pair[1].parse().unwrap());
        let found = detect::boxes(&img, at);
        println!("{at:?} -> {found:?}");
        for (k, b) in found.iter().enumerate() {
            let c = colors[k % colors.len()];
            for t in 0..3u32 {
                for x in b[0]..b[0] + b[2] {
                    for y in [b[1] + t, (b[1] + b[3]).saturating_sub(1 + t)] {
                        let i = ((y.min(img.h - 1) * img.w + x.min(img.w - 1)) * 4) as usize;
                        out.rgba[i..i + 3].copy_from_slice(&c);
                    }
                }
                for y in b[1]..b[1] + b[3] {
                    for x in [b[0] + t, (b[0] + b[2]).saturating_sub(1 + t)] {
                        let i = ((y.min(img.h - 1) * img.w + x.min(img.w - 1)) * 4) as usize;
                        out.rgba[i..i + 3].copy_from_slice(&c);
                    }
                }
            }
        }
    }
    std::fs::write(&args[1], out.encode_png().unwrap()).unwrap();
}
