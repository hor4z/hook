use hook_core::Image;

#[cfg(not(target_os = "linux"))]
pub fn monitor(connector: Option<&str>, position: (i32, i32)) -> Result<Image, String> {
    #[cfg(target_os = "linux")]
    {
        let _ = position;
        let c = connector.ok_or("unknown monitor name")?;
        crate::mutter::grab(&[c.to_string()])?.into_iter().next().flatten().ok_or_else(|| format!("no image for {c}"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = connector;
        let all = xcap::Monitor::all().map_err(|e| e.to_string())?;
        let m = all.into_iter().find(|m| (m.x().unwrap_or(0), m.y().unwrap_or(0)) == position).ok_or("monitor not found")?;
        let img = m.capture_image().map_err(|e| e.to_string())?;
        Ok(Image {
            w: img.width(),
            h: img.height(),
            rgba: img.into_raw(),
        })
    }
}

pub fn desktop(mons: &[(Option<String>, (i32, i32, u32, u32))], origin: (i32, i32), size: (u32, u32)) -> Result<Image, String> {
    let mut out = Image::new(size.0, size.1);
    #[cfg(target_os = "linux")]
    let shots: Vec<Option<Image>> = {
        let names: Vec<String> = mons.iter().map(|m| m.0.clone().unwrap_or_default()).collect();
        crate::mutter::grab(&names)?
    };
    #[cfg(not(target_os = "linux"))]
    let shots: Vec<Option<Image>> = mons.iter().map(|m| monitor(m.0.as_deref(), (m.1.0, m.1.1)).ok()).collect();
    let mut any = false;
    for ((_, r), shot) in mons.iter().zip(shots) {
        let Some(img) = shot else { continue };
        let img = img.resize(r.2, r.3);
        let (ox, oy) = ((r.0 - origin.0).max(0) as u32, (r.1 - origin.1).max(0) as u32);
        for y in 0..img.h.min(size.1.saturating_sub(oy)) {
            let w = img.w.min(size.0.saturating_sub(ox)) as usize * 4;
            let src = (y * img.w * 4) as usize;
            let dst = (((oy + y) * size.0 + ox) * 4) as usize;
            out.rgba[dst..dst + w].copy_from_slice(&img.rgba[src..src + w]);
        }
        any = true;
    }
    if any { Ok(out) } else { Err("no monitor images".into()) }
}
