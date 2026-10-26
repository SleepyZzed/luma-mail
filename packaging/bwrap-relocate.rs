//! Translate WebKit's relocated compiled paths before entering its sandbox.
//! A relative bind destination is interpreted from the sandbox root, not the
//! AppImage working directory. Never pass those destinations to real bubblewrap.
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read},
    os::{
        fd::FromRawFd,
        unix::{
            ffi::{OsStrExt, OsStringExt},
            process::CommandExt,
        },
    },
    path::{Component, Path},
    process::Command,
};

const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn decode(bytes: &[u8]) -> io::Result<Vec<OsString>> {
    if bytes.len() > MAX_ARGUMENT_BYTES || bytes.last() != Some(&0) {
        return Err(invalid("Invalid WebKit sandbox argument data"));
    }
    Ok(bytes[..bytes.len() - 1]
        .split(|b| *b == 0)
        .map(|part| OsString::from_vec(part.to_vec()))
        .collect())
}

fn expand(arguments: Vec<OsString>) -> io::Result<Vec<OsString>> {
    let mut expanded = Vec::new();
    let mut iter = arguments.into_iter();
    let mut command = false;
    for_argument_loop(&mut iter, &mut expanded, &mut command)?;
    if expanded.iter().map(|a| a.len() + 1).sum::<usize>() > MAX_ARGUMENT_BYTES {
        return Err(invalid("WebKit sandbox arguments exceed the limit"));
    }
    Ok(expanded)
}

fn for_argument_loop(
    iter: &mut impl Iterator<Item = OsString>,
    expanded: &mut Vec<OsString>,
    command: &mut bool,
) -> io::Result<()> {
    while let Some(argument) = iter.next() {
        if !*command && argument == "--args" {
            let fd = iter
                .next()
                .ok_or_else(|| invalid("Missing sandbox argument descriptor"))?;
            let fd: i32 = fd
                .to_str()
                .and_then(|s| s.parse().ok())
                .filter(|n| *n >= 3)
                .ok_or_else(|| invalid("Invalid sandbox argument descriptor"))?;
            // GLib handed this descriptor to the child. Consume it once; the
            // flattened argv no longer needs it. Other inherited FDs, including
            // WebKit's seccomp and IPC descriptors, remain open across exec.
            let file = unsafe { File::from_raw_fd(fd) };
            let mut bytes = Vec::new();
            file.take(MAX_ARGUMENT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let arguments = decode(&bytes)?;
            expanded.extend(arguments);
        } else {
            *command |= argument == "--";
            expanded.push(argument);
        }
    }
    Ok(())
}

fn relocate(argument: OsString, root: &Path) -> io::Result<OsString> {
    let bytes = argument.as_bytes();
    if !bytes.starts_with(b"././") {
        return Ok(argument);
    }
    let mut suffix = &bytes[4..];
    while suffix.first() == Some(&b'/') {
        suffix = &suffix[1..];
    }
    let relative = Path::new(OsStr::from_bytes(suffix));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || !matches!(relative.components().next(), Some(Component::Normal(first))
            if first == "lib" || first == "bin" || first == "share")
    {
        return Err(invalid("Unexpected relocated WebKit path"));
    }
    Ok(root.join(relative).into_os_string())
}

fn run() -> io::Result<()> {
    let appdir = env::var_os("APPDIR").ok_or_else(|| invalid("Missing AppImage root"))?;
    let root = Path::new(&appdir).join("usr").canonicalize()?;
    let arguments = expand(env::args_os().skip(1).collect())?
        .into_iter()
        .map(|argument| relocate(argument, &root))
        .collect::<io::Result<Vec<_>>>()?;
    // This is the original distro bubblewrap, including its namespace/seccomp
    // enforcement. The shim only makes bundle paths absolute.
    Err(Command::new(root.join("bin/bwrap.real"))
        .args(arguments)
        .exec())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Luma sandbox launcher: {error}");
        std::process::exit(125);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_and_bind_paths_are_absolute_inside_the_bundle() {
        let root = Path::new("/tmp/.mount-luma/usr");
        for (input, suffix) in [
            (
                "././/lib/x86_64-linux-gnu/webkitgtk-6.0",
                "lib/x86_64-linux-gnu/webkitgtk-6.0",
            ),
            ("././/bin/xdg-dbus-proxy", "bin/xdg-dbus-proxy"),
            (
                "././/lib/x86_64-linux-gnu/webkitgtk-6.0/WebKitWebProcess",
                "lib/x86_64-linux-gnu/webkitgtk-6.0/WebKitWebProcess",
            ),
        ] {
            assert_eq!(
                relocate(input.into(), root).unwrap(),
                root.join(suffix).into_os_string()
            );
        }
        for input in ["--ro-bind-try", "/usr/lib", "/tmp/wayland-0", "./ordinary"] {
            assert_eq!(relocate(input.into(), root).unwrap(), input);
        }
        assert!(relocate("././/lib/../../etc".into(), root).is_err());
    }

    #[test]
    fn fd_payload_preserves_empty_values_and_rejects_truncation() {
        let args = decode(
            b"--setenv\0EMPTY\0\0--ro-bind\0././/bin/xdg-dbus-proxy\0././/bin/xdg-dbus-proxy\0",
        )
        .unwrap();
        assert_eq!(args[2], "");
        assert_eq!(args.len(), 6);
        assert!(decode(b"--ro-bind").is_err());
        assert!(decode(&vec![0; MAX_ARGUMENT_BYTES + 1]).is_err());
    }

    #[test]
    fn consumes_real_argument_fd_and_keeps_application_arguments() {
        use std::{
            io::Write,
            os::{fd::IntoRawFd, unix::net::UnixStream},
        };
        let (mut writer, reader) = UnixStream::pair().unwrap();
        writer
            .write_all(b"--ro-bind\0././/bin/xdg-dbus-proxy\0././/bin/xdg-dbus-proxy\0")
            .unwrap();
        drop(writer);
        let fd = reader.into_raw_fd();
        let args = expand(vec![
            "--args".into(),
            fd.to_string().into(),
            "--".into(),
            "helper".into(),
            "--args".into(),
            "not-an-fd".into(),
        ])
        .unwrap();
        assert_eq!(args.len(), 7);
        assert_eq!(args[0], "--ro-bind");
        assert_eq!(args[5], "--args");
    }
}
