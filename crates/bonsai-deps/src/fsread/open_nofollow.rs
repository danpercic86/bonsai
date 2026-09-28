//! `[Q9-5]` opening the FINAL path component without following a symlink,
//! plus the handle-vs-`lstat` identity check. Hard-coded flag values (no
//! `libc` / `windows-sys` edge, no `unsafe`); on an unlisted target the
//! flag is 0 and only the identity check guards the open.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// `O_NOFOLLOW`, per `<fcntl.h>`: Linux asm-generic `0o400000` (x86,
/// x86_64, riscv64, loongarch64, s390x); `0o100000` on Linux arm / aarch64
/// / powerpc (`arch/*/include/uapi/asm/fcntl.h`); `0x0100` on macOS / iOS /
/// the BSDs (`sys/fcntl.h`).
#[cfg(unix)]
const O_NOFOLLOW: i32 = if cfg!(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)) {
    0x0100
} else if cfg!(all(
    target_os = "linux",
    any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
)) {
    0o100000
} else if cfg!(target_os = "linux") {
    0o400000
} else {
    0
};

/// `FILE_FLAG_OPEN_REPARSE_POINT` (`WinBase.h`): open the reparse point
/// itself, so a symlink / junction yields a handle that is not a file.
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// Opens `path` read-only without following a symlink in its LAST
/// component (a link ⇒ an error on Unix, a non-file handle on Windows).
pub(crate) fn open_nofollow(path: &Path) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    o.open(path)
}

/// Whether two metadata describe the same file: `(dev, ino)` on Unix; on
/// Windows std exposes no stable file id, so length + mtime + ctime.
pub(crate) fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len()
            && a.modified().ok() == b.modified().ok()
            && a.created().ok() == b.created().ok()
    }
}
