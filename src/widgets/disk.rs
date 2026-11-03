//! How full a filesystem is. For /tmp: the per-user quota where there is one.

use super::{icon, Reading, Widget};
use crate::meter::Gauge;
use std::ffi::CString;

pub struct Disk {
    path: CString,
    quota: bool,
}

impl Disk {
    pub fn new(path: &str, quota: bool) -> Disk {
        Disk { path: CString::new(path).unwrap_or_else(|_| CString::new("/").unwrap()), quota }
    }
}

fn filesystem(path: &CString) -> Option<u32> {
    let mut fs: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut fs) } != 0 || fs.f_blocks == 0 {
        return None;
    }
    Some((100.0 * (1.0 - fs.f_bavail as f64 / fs.f_blocks as f64)) as u32)
}

/// `struct if_dqblk` from <linux/quota.h>.
#[repr(C)]
#[derive(Default)]
struct DqBlk {
    bhardlimit: u64,
    bsoftlimit: u64,
    curspace: u64,
    ihardlimit: u64,
    isoftlimit: u64,
    curinodes: u64,
    btime: u64,
    itime: u64,
    valid: u32,
}

/// On a tmpfs with a per-user quota (Omarchy's /tmp) the quota runs out long
/// before the filesystem does: writes fail with EDQUOT while df shows room.
fn user_quota(path: &CString) -> Option<u32> {
    const Q_GETQUOTA: libc::c_int = 0x800007;
    const USRQUOTA: libc::c_int = 0;
    const QIF_BLIMITS: u32 = 1;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    let mut q = DqBlk::default();
    let cmd = (Q_GETQUOTA << 8) | (USRQUOTA & 0xff);
    let ok = unsafe { libc::syscall(libc::SYS_quotactl_fd, fd, cmd, libc::getuid(), &mut q as *mut DqBlk) } == 0;
    unsafe { libc::close(fd) };
    // Limits are in 1 KiB blocks, usage in bytes.
    (ok && q.valid & QIF_BLIMITS != 0 && q.bhardlimit > 0)
        .then(|| (q.curspace * 100 / (q.bhardlimit * 1024)) as u32)
}

impl Widget for Disk {
    fn read(&mut self) -> Reading {
        let percent = if self.quota { user_quota(&self.path) } else { None }.or_else(|| filesystem(&self.path));
        let (icon, name) = if self.quota { (icon::TMP, "TMP") } else { (icon::DISK, "SSD") };
        Reading { icon: Some(icon.into()), name: Some(name.into()), gauge: percent.map(Gauge::new), extra: None }
    }
}
