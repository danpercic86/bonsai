//! `[Q9-5]` opening the FINAL path component without following a symlink,
//! plus the handle-vs-`lstat` identity check. Hard-coded flag values (no
//! `libc` / `windows-sys` edge, no `unsafe`). A Unix target whose value is
//! not listed below is a `compile_error!` — never a silent `0` flag, which
//! would make the final open follow symlinks (fail-open).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// `O_NOFOLLOW`, per `<fcntl.h>`: `0x0100` on the Darwin family (macOS /
/// iOS / tvOS / watchOS / visionOS) and the BSDs (`sys/fcntl.h`).
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const O_NOFOLLOW: i32 = 0x0100;

/// `O_NOFOLLOW` on the Linux kernel (Linux + Android) for arm / aarch64 /
/// powerpc / m68k: `0o100000` (`arch/*/include/uapi/asm/fcntl.h`).
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "m68k"
    )
))]
const O_NOFOLLOW: i32 = 0o100000;

/// `O_NOFOLLOW` on the Linux kernel (Linux + Android), asm-generic
/// `0o400000` (x86, x86_64, riscv, loongarch64, s390x; mips and sparc
/// define the same `0x20000`).
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    )
))]
const O_NOFOLLOW: i32 = 0o400000;

// Fail closed: every Unix target must match exactly one arm above. This
// predicate is the negation of their union — keep the two in sync.
#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
        all(
            any(target_os = "linux", target_os = "android"),
            any(
                target_arch = "arm",
                target_arch = "aarch64",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "m68k",
                target_arch = "x86",
                target_arch = "x86_64",
                target_arch = "riscv32",
                target_arch = "riscv64",
                target_arch = "loongarch64",
                target_arch = "s390x",
                target_arch = "mips",
                target_arch = "mips64",
                target_arch = "sparc",
                target_arch = "sparc64"
            )
        )
    ))
))]
compile_error!(
    "bonsai-deps: no known O_NOFOLLOW value for this Unix target; add it to \
     fsread/open_nofollow.rs (a 0 fallback would make the open follow symlinks)"
);

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
