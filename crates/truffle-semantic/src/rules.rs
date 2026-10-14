//! What gets indexed, and as what (TRD §6.7): by extension, with screenshots told apart by name and folder, secrets
//! never read, and real MPEG transport streams told from TypeScript. The folder rules (skipped folders, code projects,
//! datasets) arrive with the crawler in M6.

use std::path::Path;

use crate::{FileKind, IndexOptions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Classification {
    Index(FileKind),
    /// A reason worth reporting ("code or data file").
    Skip(&'static str),
    /// A type Truffle doesn't handle at all.
    Ignore,
}

/// Pictures every build reads (the image crate, resvg). macOS's ImageIO adds HEIC, AVIF and RAW.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "bmp", "dib", "tif", "tiff", "webp", "ico", "svg",
];
/// Pictures only the system decoder reads: macOS ImageIO.
pub const SYSTEM_IMAGE_EXTENSIONS: &[&str] = &[
    "heic", "heif", "avif", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "dng", "raf", "orf", "rw2", "rwl",
    "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw",
];
pub const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "dng", "raf", "orf", "rw2", "rwl",
    "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw",
];
pub const DOC_EXTENSIONS: &[&str] = &[
    "txt", "text", "md", "markdown", "rtf", "docx", "doc", "odt", "html", "htm",
];
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "m4b", "aac", "wav", "wave", "flac", "ogg", "oga", "opus", "aiff", "aif", "wma",
    "amr", "caf", "mka",
];
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "flv", "3gp", "3g2", "mpg", "mpeg", "m2v",
    "ogv", "mxf",
];

/// Source code and data files: off outside code search (an editor and ripgrep search them better, and they flood an
/// index).
pub const CODE_EXTENSIONS: &[&str] = &[
    "py", "ipynb", "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "m", "mm", "swift", "tsx", "cts",
    "js", "jsx", "mjs", "cjs", "go", "rs", "java", "kt", "kts", "scala", "rb", "php", "cs", "fs",
    "sh", "bash", "zsh", "fish", "ps1", "psm1", "bat", "cmd", "lua", "pl", "r", "jl", "dart",
    "sql", "vue", "svelte", "css", "scss", "sass", "less", "json", "jsonl", "yaml", "yml", "toml",
    "xml", "plist", "gradle", "cmake", "mk", "lock", "log", "csv", "tsv", "ini", "cfg", "conf",
    "proto", "graphql", "tf", "nix", "zig", "hs", "ex", "exs", "erl", "clj", "elm", "ml", "vim",
    "el", "asm", "metal", "glsl", "hlsl", "wgsl", "cu", "pyi", "pyc", "xaml", "csproj", "vcxproj",
    "sln", "props", "targets",
];

// MARK: Code (the code index, `IndexOptions::code`)

/// Source files code search reads: the reference's list, and Windows' own (PowerShell modules, batch files, XAML,
/// Razor, VB, F# scripts). Project files (.csproj, .sln) stay data.
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "py", "pyi", "pyx", "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx", "m", "mm", "swift", "ts", "tsx", "mts",
    "cts", "js", "jsx", "mjs", "cjs", "go", "rs", "java", "kt", "kts", "scala", "groovy", "rb", "php", "cs", "fs",
    "sh", "bash", "zsh", "fish", "ps1", "lua", "pl", "pm", "r", "jl", "dart", "sql", "vue", "svelte", "astro",
    "html", "htm", "css", "scss", "sass", "less", "gradle", "cmake", "mk", "proto", "graphql", "gql", "tf", "hcl",
    "nix", "zig", "hs", "ex", "exs", "erl", "clj", "cljs", "elm", "ml", "mli", "vim", "el", "lisp", "asm", "s",
    "metal", "glsl", "hlsl", "wgsl", "cu", "cuh", "ino", "v", "sv", "tex", "psm1", "bat", "cmd", "xaml", "razor",
    "cshtml", "vb", "fsx",
];
/// What goes with code: a repo's docs and its settings.
pub const CODE_TEXT_EXTENSIONS: &[&str] = &["md", "markdown", "rst", "adoc", "org", "txt", "yaml", "yml", "toml"];
/// Code known by its name alone.
pub const CODE_FILE_NAMES: &[&str] = &[
    "Makefile", "makefile", "GNUmakefile", "Dockerfile", "Containerfile", "Justfile", "justfile", "Rakefile",
    "Gemfile", "Podfile", "Brewfile", "Procfile", "Vagrantfile", "Jenkinsfile",
];
/// Folders code search doesn't go into, besides what git ignores: other people's code kept in a repo (vendor\,
/// third_party\), what builds and runs leave (build\, dist\, target\, obj\), for repos whose .gitignore doesn't list
/// them and folders that aren't in git at all.
pub const CODE_SKIPPED_FOLDER_NAMES: &[&str] = &[
    "vendor", "vendored", "third_party", "third-party", "thirdparty", "3rdparty", "external", "extern", "deps",
    "build", "dist", "out", "target", "obj", "coverage", "htmlcov", "generated", "__generated__", "wandb", "mlruns",
    "lightning_logs",
];

/// What code search reads in the code folders: source files, a repo's docs and settings, and notebooks. Never data
/// (json, csv, lockfiles, logs), minified bundles, licenses, or anything that looks like a key or a password file.
pub fn classify_code(path: &Path, options: &IndexOptions) -> Classification {
    let ext = extension(path);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if options.excluded_extensions.contains(&ext) {
        return Classification::Skip("excluded type");
    }
    if is_secret(name) {
        return Classification::Skip("key or password file");
    }
    let lower = name.to_lowercase();
    if lower.ends_with(".min.js") || lower.ends_with(".min.mjs") || lower.ends_with(".min.css") {
        return Classification::Skip("minified");
    }
    if ["license", "licence", "copying", "notice"].iter().any(|p| lower.starts_with(p)) {
        return Classification::Skip("license");
    }
    if matches!(ext.as_str(), "ts" | "mts") && is_mpeg_transport_stream(path) {
        return Classification::Ignore; // a video
    }
    if SOURCE_EXTENSIONS.contains(&ext.as_str())
        || CODE_TEXT_EXTENSIONS.contains(&ext.as_str())
        || ext == "ipynb"
        || CODE_FILE_NAMES.contains(&name)
        || name.starts_with("Dockerfile")
    {
        return Classification::Index(FileKind::Code);
    }
    if CODE_EXTENSIONS.contains(&ext.as_str()) {
        return Classification::Skip("data file"); // json, csv, lockfiles, logs, project files
    }
    Classification::Ignore
}

/// Why code search doesn't go into a folder, or None: vendored or built code, or a submodule or a worktree (".git"
/// is a file there: someone else's code, or a copy of a repo's own).
pub fn code_skip_reason(folder: &Path) -> Option<&'static str> {
    let name = folder.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if CODE_SKIPPED_FOLDER_NAMES.iter().any(|skipped| skipped.eq_ignore_ascii_case(name)) {
        return Some("vendored or built code");
    }
    if folder.join(".git").is_file() {
        return Some("submodule or worktree");
    }
    None
}

/// Snipping Tool, Win+PrtScn ("Screenshot (12).png"), Game Bar, ShareX, Greenshot, Lightshot and macOS names.
const SCREENSHOT_PREFIXES: &[&str] = &[
    "screenshot",
    "screen shot",
    "cleanshot",
    "scr-",
    "snip",
    "screen recording",
    "capture d",
    "greenshot",
    "lightshot",
];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

/// What `path` is for the main index.
pub fn classify(path: &Path, options: &IndexOptions) -> Classification {
    let ext = extension(path);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if options.excluded_extensions.contains(&ext) {
        return Classification::Skip("excluded type");
    }
    if is_secret(name) {
        return Classification::Skip("key or password file");
    }
    // .ts / .mts are usually TypeScript. Only a real transport stream is video.
    if matches!(ext.as_str(), "ts" | "mts" | "m2ts") {
        if is_mpeg_transport_stream(path) {
            return Classification::Index(FileKind::Video);
        }
        return Classification::Skip("code or data file");
    }
    if DOC_EXTENSIONS.contains(&ext.as_str()) {
        return Classification::Index(FileKind::Doc);
    }
    if CODE_EXTENSIONS.contains(&ext.as_str()) {
        return if options.include_code {
            Classification::Index(FileKind::Doc)
        } else {
            Classification::Skip("code or data file")
        };
    }
    if ext == "pdf" {
        return Classification::Index(FileKind::Pdf);
    }
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) || (cfg!(target_os = "macos") && SYSTEM_IMAGE_EXTENSIONS.contains(&ext.as_str())) {
        return Classification::Index(if is_screenshot(path) {
            FileKind::Screenshot
        } else {
            FileKind::Image
        });
    }
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        return Classification::Index(FileKind::Video);
    }
    if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        return Classification::Index(FileKind::Audio);
    }
    Classification::Ignore
}

/// Screenshots by name, or by the folder Windows saves them in (Pictures\Screenshots; Game Bar's Videos\Captures).
pub fn is_screenshot(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if SCREENSHOT_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return true;
    }
    path.parent()
        .and_then(Path::file_name)
        .and_then(|folder| folder.to_str())
        .is_some_and(|folder| {
            folder.eq_ignore_ascii_case("screenshots") || folder.eq_ignore_ascii_case("captures")
        })
}

/// Keys, certificates, password and credential files: never read, wherever they are.
pub fn is_secret(name: &str) -> bool {
    let lower = name.to_lowercase();
    let ext = lower
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or_default();
    const SECRET_TYPES: &[&str] = &[
        "pem",
        "key",
        "p12",
        "pfx",
        "keystore",
        "jks",
        "ppk",
        "kdbx",
        "kdb",
        "gpg",
        "asc",
        "crt",
        "cer",
        "der",
        "p8",
        "mobileprovision",
        "ovpn",
        "rdp",
    ];
    if SECRET_TYPES.contains(&ext) {
        return true;
    }
    const SECRET_NAMES: &[&str] = &[
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        ".env",
        "secret",
        "credential",
        ".netrc",
        ".npmrc",
        ".pypirc",
        "htpasswd",
        ".htpasswd",
    ];
    SECRET_NAMES.iter().any(|prefix| lower.starts_with(prefix))
}

/// MPEG-TS packets start with 0x47 every 188 bytes (or every 192 with a 4-byte timecode, as in .m2ts / AVCHD .mts).
pub fn is_mpeg_transport_stream(path: &Path) -> bool {
    use std::io::Read;
    let mut bytes = [0u8; 600];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut read = 0;
    while read < bytes.len() {
        match file.read(&mut bytes[read..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => read += n,
        }
    }
    read >= 400
        && ((bytes[0] == 0x47 && bytes[188] == 0x47 && bytes[376] == 0x47)
            || (bytes[4] == 0x47 && bytes[196] == 0x47 && bytes[388] == 0x47))
}

// MARK: Folders

/// Folders never worth walking into, by name (case-insensitive). Hidden and system folders are skipped by the walk.
pub const SKIPPED_FOLDER_NAMES: &[&str] = &[
    "node_modules",
    "bower_components",
    "venv",
    ".venv",
    "site-packages",
    "__pycache__",
    "deriveddata",
    "pods",
    "carthage",
    ".git",
    ".svn",
    ".hg",
    "$recycle.bin",
    "lost+found",
    "cache",
    "caches",
    ".cache",
];

/// Any of these in a folder makes it a code project, where only screenshots are indexed (assets, fixtures and
/// datasets aren't what people look for).
const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "package.swift",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "gemfile",
    "composer.json",
    "cmakelists.txt",
    "global.json",
    "directory.build.props",
];
/// Project files known by their extension (Visual Studio solutions and projects, Xcode).
const PROJECT_EXTENSIONS: &[&str] = &[
    "sln",
    "csproj",
    "vcxproj",
    "fsproj",
    "vbproj",
    "xcodeproj",
    "xcworkspace",
];

/// Why the walk doesn't go into `folder`, or None. `system` holds Windows' own folders (the Windows folder, Program
/// Files, ProgramData, AppData) and Truffle's data folder; `excluded` the folders the person skipped.
pub fn skip_reason_for_folder(
    folder: &Path,
    system: &[String],
    excluded: &[String],
) -> Option<&'static str> {
    let name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if name.ends_with(".noindex") {
        return Some(".noindex folder");
    }
    if is_skipped_name(&name) {
        return Some("developer, system or cache folder");
    }
    let text = folder.to_string_lossy();
    let key = crate::store::path_key(&text);
    if system.iter().any(|s| crate::store::path_key(s) == key) {
        return Some("system or app folder");
    }
    // Bundles (apps, photo libraries) are opaque folders macOS shows as one file.
    if [".app", ".photoslibrary", ".musiclibrary", ".tvlibrary", ".bundle", ".framework", ".xcarchive"].iter().any(|b| name.ends_with(b)) {
        return Some("app or bundle");
    }
    if excluded.iter().any(|s| {
        let s = crate::store::path_key(s);
        key == s || key.starts_with(&format!("{s}/"))
    }) {
        return Some("excluded folder");
    }
    None
}

/// The system's own folders and Truffle's data, never walked even inside a chosen folder (someone may choose /).
pub fn system_folders() -> Vec<String> {
    let mut out: Vec<String> = if cfg!(target_os = "macos") {
        ["/System", "/Library", "/Applications", "/private", "/usr", "/bin", "/sbin", "/opt", "/cores", "/dev", "/Volumes"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        ["/proc", "/sys", "/dev", "/run", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/etc", "/var", "/boot", "/opt", "/snap", "/tmp"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    if let Some(home) = crate::paths::home() {
        // App data: macOS's ~/Library; Linux's ~/.local, ~/.cache and ~/.config are hidden, skipped anyway.
        out.push(home.join("Library").to_string_lossy().into_owned());
    }
    out.push(crate::paths::data_root().to_string_lossy().into_owned());
    out
}

/// Folder names never walked (lowercase): the fixed list, and the shapes package managers and tools give their
/// caches and metadata (`uv-cache`, `pip_cache`, `requests-2.32.5.dist-info`, `foo.egg-info`).
pub fn is_skipped_name(lower: &str) -> bool {
    SKIPPED_FOLDER_NAMES.contains(&lower)
        || lower.ends_with("-cache")
        || lower.ends_with("_cache")
        || lower.ends_with(".dist-info")
        || lower.ends_with(".egg-info")
}

/// Whether a change in `dir` (inside `root`) can matter to the meaning index: not in a hidden, cache, dependency or
/// build folder, and not inside a code project (whose files only code search reads). Checked on every change the
/// watcher reports, so a busy repo (a build writing `target/`) doesn't wake the indexer.
pub fn change_matters(dir: &Path, root: &Path) -> bool {
    let Ok(rest) = dir.strip_prefix(root) else { return false };
    for part in rest.iter().filter_map(|p| p.to_str()) {
        let lower = part.to_lowercase();
        if part.starts_with('.')
            || lower.ends_with(".noindex")
            || is_skipped_name(&lower)
            || CODE_SKIPPED_FOLDER_NAMES.contains(&lower.as_str())
        {
            return false;
        }
    }
    // A project marker in the folder or any folder above it, up to the root.
    let mut here = Some(dir);
    while let Some(folder) = here.filter(|f| f.starts_with(root)) {
        // The markers are compared case-insensitively (Cargo.toml, cargo.toml): Linux disks keep case.
        if folder != root
            && std::fs::read_dir(folder).is_ok_and(|entries| {
                let names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                is_project_root(names.iter().map(String::as_str))
            })
        {
            return false;
        }
        here = folder.parent();
    }
    true
}

/// Whether `entries` (a folder's file and folder names) make it a code project.
pub fn is_project_root<'a>(entries: impl IntoIterator<Item = &'a str>) -> bool {
    entries.into_iter().any(|name| {
        let lower = name.to_lowercase();
        PROJECT_MARKERS.contains(&lower.as_str())
            || lower
                .rsplit_once('.')
                .is_some_and(|(_, ext)| PROJECT_EXTENSIONS.contains(&ext))
    })
}

static HEX_NAME: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[0-9a-f]{8,}([-_][0-9a-f]{4,})*$").unwrap());
static NUMBERED_NAME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^[a-z]{0,16}[-_. ]?\d{3,}([-_.]\d+)*$").unwrap()
});
const CAMERA_PREFIXES: &[&str] = &[
    "img",
    "dsc",
    "pxl",
    "mvimg",
    "vid",
    "gopr",
    "dji",
    "photo",
    "whatsapp",
    "screenshot",
    "wp_",
    "snapchat",
];

/// Machine-made names like "clip_000123", "frame-0042" or "3f9a1c0e-…", but not camera names like "IMG_1234".
pub fn looks_machine_named(stem: &str) -> bool {
    let name = stem.to_lowercase();
    if CAMERA_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return false;
    }
    HEX_NAME.is_match(&name) || NUMBERED_NAME.is_match(&name)
}

/// Folders with hundreds of same-kind, machine-named media files: training data, not someone's own files. They're
/// skipped (and reported) instead of indexed silently. A folder the person chose itself is never a dataset.
/// `files`: (folder, file stem, kind). Returns folder → files.
pub fn dataset_folders(
    files: &[(String, String, FileKind)],
    roots: &[String],
    minimum: usize,
) -> std::collections::HashMap<String, usize> {
    use std::collections::HashMap;
    let root_keys: Vec<String> = roots.iter().map(|r| crate::store::path_key(r)).collect();
    let mut groups: HashMap<(String, FileKind), Vec<&str>> = HashMap::new();
    for (folder, stem, kind) in files {
        if matches!(kind, FileKind::Image | FileKind::Audio | FileKind::Video)
            && !root_keys.contains(&crate::store::path_key(folder))
        {
            groups
                .entry((folder.clone(), *kind))
                .or_default()
                .push(stem);
        }
    }
    let mut datasets: HashMap<String, usize> = HashMap::new();
    for ((folder, _), stems) in groups {
        if stems.len() >= minimum {
            let machine = stems
                .iter()
                .filter(|stem| looks_machine_named(stem))
                .count();
            if machine as f64 / stems.len() as f64 >= 0.7 {
                *datasets.entry(folder).or_default() += stems.len();
            }
        }
    }
    datasets
}

/// The document title the model sees for a file: its name without the extension.
pub fn title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_folder_types() {
        let code = |name: &str| classify_code(Path::new(&format!("/repo/{name}")), &IndexOptions::default());
        for name in ["retry.py", "App.swift", "README.md", "settings.yaml", "explore.ipynb", "Makefile",
                     "Dockerfile.dev", "app.ts", "Build.ps1", "Module.psm1", "MainWindow.xaml"] {
            assert_eq!(code(name), Classification::Index(FileKind::Code), "{name}");
        }
        assert_eq!(code("package.json"), Classification::Skip("data file"));
        assert_eq!(code("rows.csv"), Classification::Skip("data file"));
        assert_eq!(code("yarn.lock"), Classification::Skip("data file"));
        assert_eq!(code("App.csproj"), Classification::Skip("data file"));
        assert_eq!(code("bundle.min.js"), Classification::Skip("minified"));
        assert_eq!(code("LICENSE.md"), Classification::Skip("license"));
        assert_eq!(code("prod.pem"), Classification::Skip("key or password file"));
        assert_eq!(code("id_ed25519"), Classification::Skip("key or password file"));
        assert_eq!(code("credentials.yaml"), Classification::Skip("key or password file"));
        assert_eq!(code("photo.jpg"), Classification::Ignore);
        assert_eq!(code_skip_reason(Path::new("/repo/vendor")), Some("vendored or built code"));
        assert_eq!(code_skip_reason(Path::new("/repo/Build")), Some("vendored or built code"));
        assert_eq!(code_skip_reason(Path::new("/repo/src")), None);
    }

    fn kind(path: &str) -> Classification {
        classify(Path::new(path), &IndexOptions::default())
    }

    #[test]
    fn kinds_by_extension_and_name() {
        // HEIC needs the system decoder (macOS ImageIO); Linux has no pure-Rust one.
        assert_eq!(
            kind("/p/IMG_1.HEIC"),
            if cfg!(target_os = "macos") { Classification::Index(FileKind::Image) } else { Classification::Ignore }
        );
        assert_eq!(kind("/p/IMG_2.jpg"), Classification::Index(FileKind::Image));
        assert_eq!(
            kind("/p/Screenshot (12).png"),
            Classification::Index(FileKind::Screenshot)
        );
        assert_eq!(
            kind("/Users/me/Pictures/Screenshots/anything.png"),
            Classification::Index(FileKind::Screenshot)
        );
        assert_eq!(
            kind("/d/lease.PDF"),
            Classification::Index(FileKind::Pdf)
        );
        assert_eq!(
            kind("/d/notes.docx"),
            Classification::Index(FileKind::Doc)
        );
        assert_eq!(
            kind("/d/main.rs"),
            Classification::Skip("code or data file")
        );
        assert_eq!(
            kind("/d/app.ts"),
            Classification::Skip("code or data file")
        ); // not a file: not a stream
        assert_eq!(
            kind("/d/song.flac"),
            Classification::Index(FileKind::Audio)
        );
        assert_eq!(
            kind("/d/clip.mkv"),
            Classification::Index(FileKind::Video)
        );
        assert_eq!(kind("/d/archive.zip"), Classification::Ignore);
        assert_eq!(
            kind("/d/server.pem"),
            Classification::Skip("key or password file")
        );
        assert_eq!(
            kind("/d/.env.local"),
            Classification::Skip("key or password file")
        );
        assert_eq!(title(Path::new("/d/Lease 2026.pdf")), "Lease 2026");
    }

    #[test]
    fn excluded_types() {
        let options = IndexOptions {
            excluded_extensions: ["heic".to_string()].into(),
            ..IndexOptions::default()
        };
        assert_eq!(
            classify(Path::new("a.heic"), &options),
            Classification::Skip("excluded type")
        );
    }

    #[test]
    fn changes_that_matter() {
        let root = std::env::temp_dir().join(format!("truffle-changes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("app/src")).unwrap();
        std::fs::write(root.join("app/Cargo.toml"), "").unwrap();
        std::fs::create_dir_all(root.join("Photos/2026")).unwrap();
        assert!(change_matters(&root.join("Photos/2026"), &root));
        assert!(change_matters(&root, &root));
        assert!(!change_matters(&root.join("app/src"), &root));
        assert!(!change_matters(&root.join("app/target/release"), &root));
        assert!(!change_matters(&root.join("Photos/node_modules/x"), &root));
        assert!(!change_matters(&root.join(".cache/x"), &root));
        assert!(!change_matters(Path::new("/elsewhere"), &root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn folder_rules() {
        let system = vec!["/usr".to_string()];
        let excluded = vec!["/mnt/d/Photos/Old".to_string()];
        let reason = |p: &str| skip_reason_for_folder(Path::new(p), &system, &excluded);
        assert_eq!(
            reason("/mnt/d/code/app/node_modules"),
            Some("developer, system or cache folder")
        );
        assert_eq!(reason("/usr"), Some("system or app folder"));
        assert_eq!(reason("/Users/me/Applications/Foo.app"), Some("app or bundle"));
        assert_eq!(reason("/mnt/d/Photos/Old/2019"), Some("excluded folder"));
        assert_eq!(reason("/mnt/d/Photos/Older"), None);
        assert_eq!(reason("/w/uv-cache"), Some("developer, system or cache folder"));
        assert_eq!(reason("/w/requests-2.32.5.dist-info"), Some("developer, system or cache folder"));
        assert_eq!(reason("/mnt/d/archive.noindex"), Some(".noindex folder"));
        assert!(is_project_root(["src", "Cargo.toml"]));
        assert!(is_project_root(["App.sln", "README.md"]));
        assert!(!is_project_root(["IMG_1.jpg", "notes.txt"]));
    }

    #[test]
    fn machine_names_and_datasets() {
        assert!(looks_machine_named("clip_000123"));
        assert!(looks_machine_named("frame-0042"));
        assert!(looks_machine_named("3f9a1c0e-4b2a"));
        assert!(!looks_machine_named("IMG_1234"));
        assert!(!looks_machine_named("beach day"));
        let files: Vec<(String, String, FileKind)> = (0..400)
            .map(|i| {
                (
                    "/mnt/d/data/train".to_string(),
                    format!("sample_{i:05}"),
                    FileKind::Image,
                )
            })
            .chain((0..400).map(|i| {
                (
                    "/mnt/d/Pictures/Trip".to_string(),
                    format!("IMG_{i:04}"),
                    FileKind::Image,
                )
            }))
            .collect();
        let datasets = dataset_folders(&files, &["/mnt/d/Pictures".to_string()], 300);
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets["/mnt/d/data/train"], 400);
        // A folder chosen on its own is never a dataset.
        assert!(dataset_folders(&files, &["/mnt/d/data/train".to_string()], 300).is_empty());
    }
}
