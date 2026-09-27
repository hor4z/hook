use std::io;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(w: u32, h: u32) -> Image {
        Image {
            w,
            h,
            rgba: vec![0; (w * h * 4) as usize],
        }
    }

    pub fn decode_png(bytes: &[u8]) -> io::Result<Image> {
        let mut dec = png::Decoder::new(io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().map_err(io::Error::other)?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| io::Error::other("png too large"))?];
        let info = reader.next_frame(&mut buf).map_err(io::Error::other)?;
        let px = &buf[..info.buffer_size()];
        let rgba = match info.color_type {
            png::ColorType::Rgba => px.to_vec(),
            png::ColorType::Rgb => px.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => px.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => return Err(io::Error::other("indexed png not expanded")),
        };
        Ok(Image { w: info.width, h: info.height, rgba })
    }

    pub fn encode_png(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(io::Error::other)?;
        writer.write_image_data(&self.rgba).map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)?;
        Ok(out)
    }

    pub fn crop(&self, x: i64, y: i64, w: i64, h: i64) -> Image {
        let x0 = x.clamp(0, self.w as i64) as u32;
        let y0 = y.clamp(0, self.h as i64) as u32;
        let x1 = (x + w).clamp(x0 as i64, self.w as i64) as u32;
        let y1 = (y + h).clamp(y0 as i64, self.h as i64) as u32;
        let (cw, ch) = (x1 - x0, y1 - y0);
        let mut rgba = Vec::with_capacity((cw * ch * 4) as usize);
        for row in y0..y1 {
            let start = ((row * self.w + x0) * 4) as usize;
            rgba.extend_from_slice(&self.rgba[start..start + (cw * 4) as usize]);
        }
        Image { w: cw, h: ch, rgba }
    }

    pub fn resize(&self, w: u32, h: u32) -> Image {
        if (w, h) == (self.w, self.h) || self.w == 0 || self.h == 0 {
            return self.clone();
        }
        let (sx, sy) = (self.w as f32 / w as f32, self.h as f32 / h as f32);
        let mut out = Image::new(w, h);
        for y in 0..h {
            let (fy0, fy1) = (y as f32 * sy, ((y + 1) as f32 * sy).max(y as f32 * sy + 1.0));
            for x in 0..w {
                let (fx0, fx1) = (x as f32 * sx, ((x + 1) as f32 * sx).max(x as f32 * sx + 1.0));
                let mut acc = [0f32; 4];
                let mut n = 0f32;
                for yy in fy0 as u32..(fy1.ceil() as u32).min(self.h) {
                    for xx in fx0 as u32..(fx1.ceil() as u32).min(self.w) {
                        let i = ((yy * self.w + xx) * 4) as usize;
                        for c in 0..4 {
                            acc[c] += self.rgba[i + c] as f32;
                        }
                        n += 1.0;
                    }
                }
                let o = ((y * w + x) * 4) as usize;
                for c in 0..4 {
                    out.rgba[o + c] = (acc[c] / n.max(1.0)).round() as u8;
                }
            }
        }
        out
    }

    pub fn fit(&self, max: u32) -> Image {
        let long = self.w.max(self.h);
        if long <= max {
            return self.clone();
        }
        let k = max as f32 / long as f32;
        self.resize(((self.w as f32 * k).round() as u32).max(1), ((self.h as f32 * k).round() as u32).max(1))
    }

    pub fn over_premultiplied(&mut self, top: &Image) {
        let (w, h) = (self.w.min(top.w), self.h.min(top.h));
        for y in 0..h {
            for x in 0..w {
                let i = ((y * self.w + x) * 4) as usize;
                let j = ((y * top.w + x) * 4) as usize;
                let a = top.rgba[j + 3] as u32;
                if a == 0 {
                    continue;
                }
                for c in 0..3 {
                    let v = top.rgba[j + c] as u32 + self.rgba[i + c] as u32 * (255 - a) / 255;
                    self.rgba[i + c] = v.min(255) as u8;
                }
                self.rgba[i + 3] = 255;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Image {
        Image {
            w,
            h,
            rgba: px.repeat((w * h) as usize),
        }
    }

    #[test]
    fn png_round_trips() {
        let img = solid(3, 2, [10, 200, 30, 255]);
        assert_eq!(Image::decode_png(&img.encode_png().unwrap()).unwrap(), img);
    }

    #[test]
    fn crop_clamps_to_the_image() {
        let mut img = Image::new(4, 4);
        img.rgba[((1 * 4 + 1) * 4) as usize] = 9;
        let c = img.crop(1, 1, 10, 2);
        assert_eq!((c.w, c.h), (3, 2));
        assert_eq!(c.rgba[0], 9);
        assert_eq!(img.crop(-5, -5, 2, 2).w, 0);
    }

    #[test]
    fn fit_keeps_aspect_and_averages() {
        let img = solid(400, 200, [100, 100, 100, 255]);
        let f = img.fit(100);
        assert_eq!((f.w, f.h), (100, 50));
        assert_eq!(&f.rgba[..4], &[100, 100, 100, 255]);
        assert_eq!(img.fit(1000).w, 400);
    }

    #[test]
    fn premultiplied_over_blends() {
        let mut base = solid(1, 1, [0, 0, 200, 255]);
        base.over_premultiplied(&solid(1, 1, [128, 0, 0, 128]));
        assert_eq!(base.rgba, vec![128, 0, 99, 255]);
        let mut keep = solid(1, 1, [1, 2, 3, 255]);
        keep.over_premultiplied(&solid(1, 1, [0, 0, 0, 0]));
        assert_eq!(keep.rgba, vec![1, 2, 3, 255]);
    }
}
