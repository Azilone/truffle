//! What differs between macOS and Linux below the index: thread priority,
//! IO hints, giving memory back, and where the data lives. Everything else
//! in the crate is plain POSIX.

/// How urgent a thread's work is. On macOS these are QoS classes (which also
/// pick performance or efficiency cores and the IO tier); on Linux, nice
/// values on the thread (a thread can lower its own priority, never raise it
/// back, so only background threads are niced).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Qos {
    Interactive,
    Initiated,
    Utility,
}

#[cfg(target_os = "macos")]
pub fn set_qos(q: Qos) {
    let class = match q {
        Qos::Interactive => libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE,
        Qos::Initiated => libc::qos_class_t::QOS_CLASS_USER_INITIATED,
        Qos::Utility => libc::qos_class_t::QOS_CLASS_UTILITY,
    };
    unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
}

#[cfg(target_os = "linux")]
pub fn set_qos(q: Qos) {
    if q != Qos::Utility {
        return;
    }
    // setpriority on a thread id nices that thread alone (a Linux quirk POSIX
    // leaves open), and the idle IO class keeps its reads behind everyone's.
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 10);
        const IOPRIO_WHO_PROCESS: libc::c_int = 1;
        const IOPRIO_CLASS_IDLE: libc::c_int = 3;
        libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, IOPRIO_CLASS_IDLE << 13);
    }
}

/// Never let this thread download iCloud placeholders: opening or listing a
/// dataless file fails fast instead of materializing it. Linux has none.
#[cfg(target_os = "macos")]
pub fn no_materialize_thread() {
    unsafe extern "C" {
        fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
    }
    // IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_THREAD, OFF
    unsafe { setiopolicy_np(3, 1, 1) };
}

#[cfg(target_os = "linux")]
pub fn no_materialize_thread() {}

/// The process-wide form of `no_materialize_thread`, for the CLI.
#[cfg(target_os = "macos")]
pub fn no_materialize_process() {
    unsafe extern "C" {
        fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
    }
    unsafe { setiopolicy_np(3, 0, 1) };
}

#[cfg(target_os = "linux")]
pub fn no_materialize_process() {}

/// Run `f` with this thread never materializing placeholders, then restore
/// its policy.
#[cfg(target_os = "macos")]
pub fn without_materializing<T>(f: impl FnOnce() -> T) -> T {
    unsafe extern "C" {
        fn getiopolicy_np(iotype: i32, scope: i32) -> i32;
        fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
    }
    let prior = unsafe { getiopolicy_np(3, 1) };
    no_materialize_thread();
    let r = f();
    if prior >= 0 {
        unsafe { setiopolicy_np(3, 1, prior) };
    }
    r
}

#[cfg(target_os = "linux")]
pub fn without_materializing<T>(f: impl FnOnce() -> T) -> T {
    f()
}

/// Start reading the first `len` bytes of an open file.
#[cfg(target_os = "macos")]
pub fn read_ahead(fd: i32, len: u64) {
    let ra = libc::radvisory { ra_offset: 0, ra_count: len.min(i32::MAX as u64) as libc::c_int };
    unsafe { libc::fcntl(fd, libc::F_RDADVISE, &ra) };
}

#[cfg(target_os = "linux")]
pub fn read_ahead(fd: i32, len: u64) {
    unsafe { libc::posix_fadvise(fd, 0, len.min(i64::MAX as u64) as libc::off_t, libc::POSIX_FADV_WILLNEED) };
}

/// Hand freed allocator memory back to the OS after big transient work.
#[cfg(target_os = "macos")]
pub fn release_memory() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn release_memory() {
    unsafe { libc::malloc_trim(0) };
}

#[cfg(all(target_os = "linux", not(target_env = "gnu")))]
pub fn release_memory() {}

/// Cores worth reading files with: the performance cores on Apple silicon
/// (efficiency cores slow a parallel grep down), every physical core elsewhere.
#[cfg(target_os = "macos")]
pub fn fast_cores() -> Option<usize> {
    let get = |name: &str| {
        let name = std::ffi::CString::new(name).ok()?;
        let (mut v, mut len) = (0i32, std::mem::size_of::<i32>());
        let ok = unsafe { libc::sysctlbyname(name.as_ptr(), (&raw mut v).cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
        (ok && v > 0).then_some(v as usize)
    };
    // The levels run fastest first; the last one is the efficiency cores.
    match get("hw.nperflevels") {
        Some(n) if n > 1 => Some((0..n - 1).filter_map(|i| get(&format!("hw.perflevel{i}.physicalcpu"))).sum()),
        _ => get("hw.physicalcpu"),
    }
}

#[cfg(target_os = "linux")]
pub fn fast_cores() -> Option<usize> {
    std::thread::available_parallelism().ok().map(|n| n.get())
}

/// Can this process read everything? On macOS, the system TCC database opens
/// only with Full Disk Access (and trying fails at once, no prompt). Linux
/// has no consent gate: unreadable folders are just skipped.
#[cfg(target_os = "macos")]
pub fn has_full_disk_access() -> bool {
    std::fs::File::open("/Library/Application Support/com.apple.TCC/TCC.db").is_ok()
}

#[cfg(target_os = "linux")]
pub fn has_full_disk_access() -> bool {
    true
}

/// Where the index lives.
#[cfg(target_os = "macos")]
pub fn default_dir(home: &str) -> std::path::PathBuf {
    std::path::Path::new(home).join("Library/Application Support/Truffle")
}

#[cfg(target_os = "linux")]
pub fn default_dir(home: &str) -> std::path::PathBuf {
    match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(d) => std::path::PathBuf::from(d).join("truffle"),
        None => std::path::Path::new(home).join(".local/share/truffle"),
    }
}
