//! What the indexer asks the system about itself and the machine: a file's identity (so a move keeps its vectors), the
//! power state, background priority, the process's memory and the local time. macOS and Linux; trovara had the
//! Windows versions of the same calls.

use std::path::Path;

/// A file's identity: its device and inode. Stays the same through moves and renames on the same disk.
pub fn file_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// The power source and savers right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Power {
    /// Running on the battery (unplugged).
    pub on_battery: bool,
    /// Low Power Mode (macOS) or the power-saver profile (Linux) is on.
    pub saver: bool,
    /// The battery's charge, 0–100, when there's a battery.
    pub percent: Option<u8>,
}

#[cfg(target_os = "macos")]
pub fn power() -> Power {
    // `pmset -g batt`: "Now drawing from 'Battery Power'" … "\t85%; discharging"; `pmset -g`: " lowpowermode 1".
    let run = |args: &[&str]| std::process::Command::new("/usr/bin/pmset").args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let batt = run(&["-g", "batt"]);
    let percent = batt.split(|c: char| c.is_whitespace() || c == ';').find_map(|w| w.strip_suffix('%')?.parse::<u8>().ok());
    let saver = run(&["-g"]).lines().any(|l| {
        let mut w = l.split_whitespace();
        w.next() == Some("lowpowermode") && w.next() == Some("1")
    });
    Power { on_battery: batt.contains("'Battery Power'"), saver, percent }
}

#[cfg(target_os = "linux")]
pub fn power() -> Power {
    let read = |p: &Path| std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    let (mut mains, mut on_mains, mut percent) = (false, false, None);
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for e in entries.flatten() {
            let p = e.path();
            match read(&p.join("type")).as_str() {
                "Mains" | "USB" => {
                    mains = true;
                    on_mains |= read(&p.join("online")) == "1";
                }
                "Battery" => percent = read(&p.join("capacity")).parse::<u8>().ok().or(percent),
                _ => {}
            }
        }
    }
    let saver = read(Path::new("/sys/firmware/acpi/platform_profile")) == "low-power";
    Power { on_battery: mains && !on_mains, saver, percent }
}

/// Indexing steps out of the way: background CPU and IO (macOS's background band; nice 10 and the idle IO class on
/// Linux). `eco` is accepted for trovara's callers; both platforms throttle the same way.
pub fn enter_background(_eco: bool) {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, libc::PRIO_DARWIN_BG as libc::c_int);
    }
    #[cfg(target_os = "linux")]
    set_below_normal(true);
}

/// Back to normal priority (loading the model, a search waiting).
pub fn leave_background() {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, 0);
    }
}

/// Lower (or restore) the whole process's CPU priority. Linux can't raise it back without privileges: restoring is a
/// no-op there, and the process exits when its work is done anyway.
pub fn set_below_normal(on: bool) {
    if on {
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
        }
        #[cfg(target_os = "linux")]
        unsafe {
            libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
        }
    }
}

/// This thread at low (or normal) priority.
pub fn set_thread_low(on: bool, _background: bool) {
    if on {
        truffle_index::sys::set_qos(truffle_index::sys::Qos::Utility);
    }
}

/// Minutes since local midnight: for "index between 22:00 and 06:00".
pub fn local_minutes() -> u32 {
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        (tm.tm_hour * 60 + tm.tm_min) as u32
    }
}

/// This process's memory: the footprint macOS charges it, or the resident set on Linux.
pub fn memory_used() -> Option<u64> {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut info: libc::rusage_info_v4 = std::mem::zeroed();
        let ok = libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, (&raw mut info).cast()) == 0;
        ok.then_some(info.ri_phys_footprint)
    }
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        Some(pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64)
    }
}

/// The machine's memory.
pub fn physical_memory() -> u64 {
    unsafe {
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES);
        let size = libc::sysconf(libc::_SC_PAGESIZE);
        if pages > 0 && size > 0 { pages as u64 * size as u64 } else { 8 << 30 }
    }
}

/// A full-screen app (a game, a presentation) in front: no such check yet on macOS or Linux.
pub fn fullscreen_busy() -> bool {
    false
}

/// Windows-only concern (DLL planting); nothing to do here.
pub fn harden_dll_search() {}
