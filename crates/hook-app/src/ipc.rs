use hook_core::ipc;
use std::io::{self, BufRead, BufReader, Write};

pub fn serve(on: impl Fn(&str) -> bool + Send + 'static) -> io::Result<()> {
    let dir = ipc::runtime_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    let listener = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        let path = ipc::socket_path();
        let _ = std::fs::remove_file(&path);
        let l = std::os::unix::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        l
    };
    #[cfg(not(unix))]
    let listener = {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        std::fs::write(ipc::port_file(), l.local_addr()?.port().to_string())?;
        l
    };
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let Ok(mut w) = stream.try_clone() else { continue };
            let mut line = String::new();
            if BufReader::new(stream).read_line(&mut line).is_err() {
                continue;
            }
            let ok = on(line.trim());
            let _ = writeln!(w, "{}", if ok { "ok" } else { "unknown command" });
        }
    });
    Ok(())
}

pub fn cleanup() {
    #[cfg(unix)]
    let _ = std::fs::remove_file(ipc::socket_path());
    #[cfg(not(unix))]
    let _ = std::fs::remove_file(ipc::port_file());
}
