use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub const SHORTCUT: &str = "Ctrl+Alt+K";
pub const MEMORIES: &str = "Ctrl+Alt+M";

pub fn hotkey(on: impl Fn() + Send + Sync + 'static, memories: impl Fn() + Send + Sync + 'static) -> Option<GlobalHotKeyManager> {
    let manager = GlobalHotKeyManager::new().ok()?;
    let key = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyK);
    manager.register(key).ok()?;
    let mem = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyM);
    let mem_id = manager.register(mem).ok().map(|_| mem.id());
    let id = key.id();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state != HotKeyState::Pressed {
            return;
        }
        if e.id == id {
            on();
        } else if Some(e.id) == mem_id {
            memories();
        }
    }));
    Some(manager)
}

#[cfg(target_os = "linux")]
pub fn gnome_shortcut() -> Result<bool, String> {
    use std::process::Command;
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !desktop.to_ascii_uppercase().contains("GNOME") || std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Ok(false);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let schema = "org.gnome.settings-daemon.plugins.media-keys";
    let get = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("gsettings").args(args).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let mut changed = false;
    for (slot, name, verb, binding) in [("hook", "Hook", "toggle", "<Control><Alt>k"), ("hook-memories", "Hook memories", "memories", "<Control><Alt>m")] {
        let command = format!("{} {verb}", exe.display());
        let path = format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{slot}/");
        let item = format!("{schema}.custom-keybinding:{path}");
        let list = get(&["get", schema, "custom-keybindings"])?;
        let current = get(&["get", &item, "command"]).unwrap_or_default();
        if list.contains(&path) && current.trim_matches('\'') == command {
            continue;
        }
        if !list.contains(&path) {
            let mut items: Vec<String> = list
                .trim_start_matches("@as")
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            items.push(format!("'{path}'"));
            get(&["set", schema, "custom-keybindings", &format!("[{}]", items.join(", "))])?;
        }
        get(&["set", &item, "name", &format!("'{name}'")])?;
        get(&["set", &item, "command", &format!("'{command}'")])?;
        get(&["set", &item, "binding", &format!("'{binding}'")])?;
        changed = true;
    }
    Ok(changed)
}

#[cfg(not(target_os = "linux"))]
pub fn gnome_shortcut() -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "linux")]
pub const APP_ID: &str = "dev.hook.Hook";

#[cfg(target_os = "linux")]
pub fn identity(args: &[String]) {
    install_desktop_entry();
    let scoped = std::fs::read_to_string("/proc/self/cgroup").is_ok_and(|c| c.contains(&format!("app-{APP_ID}")));
    if scoped || std::env::var_os("HOOK_SCOPED").is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "--collect", &format!("--unit=app-{APP_ID}-{}", std::process::id()), "--"])
        .arg(exe)
        .args(args)
        .env("HOOK_SCOPED", "1")
        .exec();
    eprintln!("hook: no dedicated scope ({err}); captures may fail");
}

#[cfg(target_os = "linux")]
fn install_desktop_entry() {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let Some(home) = env("HOME") else { return };
    let data = env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
    let Ok(exe) = std::env::current_exe() else { return };
    let icon = data.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg"));
    let desktop = data.join(format!("applications/{APP_ID}.desktop"));
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Hook\nComment=Señalá la pantalla para tu agente de código\nExec={} %U\nIcon={APP_ID}\nTerminal=false\nCategories=Development;Utility;\nStartupWMClass={APP_ID}\n",
        exe.display()
    );
    let spiral = crate::look::LOGO;
    let inner = spiral.find("<path").and_then(|a| spiral.rfind("/>").map(|b| &spiral[a..b + 2])).unwrap_or("");
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect width=\"64\" height=\"64\" rx=\"16\" fill=\"#141414\"/><g transform=\"translate(12 12) scale(0.625)\">{inner}</g></svg>\n"
    );
    if std::fs::read_to_string(&desktop).ok().as_deref() == Some(entry.as_str()) && std::fs::read_to_string(&icon).ok().as_deref() == Some(svg.as_str()) {
        return;
    }
    for (path, body) in [(&icon, svg), (&desktop, entry)] {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, body);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn identity(_: &[String]) {}
