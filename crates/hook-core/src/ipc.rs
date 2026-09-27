use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Duration;

pub fn runtime_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("hook")
}

#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    runtime_dir().join("hook.sock")
}

#[cfg(not(unix))]
pub fn port_file() -> PathBuf {
    runtime_dir().join("hook.port")
}

pub fn send(cmd: &str) -> io::Result<String> {
    #[cfg(unix)]
    let stream = std::os::unix::net::UnixStream::connect(socket_path())?;
    #[cfg(not(unix))]
    let stream = {
        let port: u16 = std::fs::read_to_string(port_file())?.trim().parse().map_err(io::Error::other)?;
        std::net::TcpStream::connect(("127.0.0.1", port))?
    };
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut w = stream.try_clone()?;
    writeln!(w, "{cmd}")?;
    w.flush()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(line.trim().to_string())
}

pub fn state_file() -> PathBuf {
    runtime_dir().join("state")
}

pub fn set_state(open: bool, typing: bool) {
    let _ = std::fs::create_dir_all(runtime_dir());
    let _ = std::fs::write(
        state_file(),
        if !open {
            "closed"
        } else if typing {
            "typing"
        } else {
            "open"
        },
    );
}

fn state() -> String {
    std::fs::read_to_string(state_file()).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn is_open() -> bool {
    matches!(state().as_str(), "open" | "typing")
}

pub fn is_typing() -> bool {
    state() == "typing"
}
