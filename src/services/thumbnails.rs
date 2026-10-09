use std::ffi::OsString;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::filesystem::TimedCache;

/// Wall-clock budget for one external renderer. `Command::output()` waited
/// forever, so a corrupt video or a file on a stalled network share pinned a
/// blocking thread for the lifetime of the process.
const EXTERNAL_TOOL_TIMEOUT: Duration = Duration::from_secs(8);

/// How often we check whether the child exited while waiting for the deadline.
const EXTERNAL_TOOL_POLL: Duration = Duration::from_millis(20);

/// How many external renderers may run at once. One task is pushed per visible
/// entry, so a folder of videos used to fork one ffmpeg per thumbnail.
const MAX_CONCURRENT_EXTERNAL_TOOLS: usize = 3;

#[derive(Debug, Clone)]
pub struct Thumbnail {
    /// Shared, not owned outright: iced's `image::Handle` keeps the encoded
    /// bytes alive for as long as it draws them. Handing it a `Vec` meant a
    /// second full copy of every thumbnail — and of every preview, which this
    /// crate caps at 64 Mio apiece.
    pub bytes: Bytes,
    pub mime: Option<String>,
}

impl Thumbnail {
    pub fn new(bytes: impl Into<Bytes>, mime: Option<String>) -> Self {
        Self {
            bytes: bytes.into(),
            mime,
        }
    }
}

#[derive(Debug)]
pub struct ThumbnailService {
    cache: TimedCache<PathBuf, Thumbnail>,
}

impl ThumbnailService {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<&Thumbnail> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, thumbnail: Thumbnail) {
        self.cache.insert(path, thumbnail);
    }

    pub fn remove(&mut self, path: &Path) {
        self.cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

#[derive(Debug)]
pub struct PreviewImageService {
    cache: TimedCache<PathBuf, Thumbnail>,
}

impl PreviewImageService {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: TimedCache::new(max_entries, ttl),
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<&Thumbnail> {
        self.cache.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, thumbnail: Thumbnail) {
        self.cache.insert(path, thumbnail);
    }

    pub fn remove(&mut self, path: &Path) {
        self.cache.remove(path);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

pub fn generate_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    // Use ImageReader to auto-detect format and apply EXIF orientation
    let reader = image::ImageReader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let image = reader.decode().ok()?;
    let thumbnail = image.thumbnail(max_size, max_size);
    let mut bytes = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .ok()?;
    Some(Thumbnail::new(bytes, Some("image/png".to_string())))
}

pub fn generate_preview(path: &Path, max_size: u32) -> Option<Thumbnail> {
    // Fast path: check extension first (free)
    if is_gif_path(path) {
        let bytes = std::fs::read(path).ok()?;
        return Some(Thumbnail::new(bytes, Some("image/gif".to_string())));
    }

    // Check 6-byte magic header; if GIF, read file once and return
    {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut header = [0u8; 6];
        if file.read_exact(&mut header).is_ok() && is_gif_header(&header) {
            // Read remaining bytes after the header we already consumed
            let mut bytes = header.to_vec();
            file.read_to_end(&mut bytes).ok()?;
            return Some(Thumbnail::new(bytes, Some("image/gif".to_string())));
        }
    }

    // Non-GIF: use ImageReader for EXIF orientation support
    let reader = image::ImageReader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let image = reader.decode().ok()?;
    let preview = image.thumbnail(max_size, max_size);
    let mut preview_bytes = Vec::new();
    preview
        .write_to(
            &mut Cursor::new(&mut preview_bytes),
            image::ImageFormat::Png,
        )
        .ok()?;
    Some(Thumbnail::new(preview_bytes, Some("image/png".to_string())))
}

/// #13: Generate video thumbnail using ffmpeg CLI (if available).
/// Extracts a single frame at 1 second into the video.
pub fn generate_video_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "mp4" | "mkv" | "avi" | "webm" | "mov" | "wmv" | "flv" | "m4v"
    ) {
        return None;
    }
    // A relative path would be resolved against the process working directory
    // and could itself begin with '-', turning the file name into an option.
    if !path.is_absolute() {
        return None;
    }
    let ffmpeg = resolve_tool(&FFMPEG, "ffmpeg")?;

    // `file:` pins the input to the file protocol: without it a name shaped like
    // `concat:` or `http:` would be read as a URL by ffmpeg.
    let mut input = OsString::from("file:");
    input.push(path.as_os_str());

    let args: Vec<OsString> = vec![
        OsString::from("-nostdin"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-protocol_whitelist"),
        OsString::from("file"),
        OsString::from("-ss"),
        OsString::from("1"),
        OsString::from("-i"),
        input,
        OsString::from("-vframes"),
        OsString::from("1"),
        OsString::from("-vf"),
        OsString::from(format!(
            "scale='min({max_size},iw)':'min({max_size},ih)':force_original_aspect_ratio=decrease"
        )),
        OsString::from("-f"),
        OsString::from("image2pipe"),
        OsString::from("-vcodec"),
        OsString::from("png"),
        OsString::from("-"),
    ];

    let bytes = run_capturing_stdout(ffmpeg, &args, EXTERNAL_TOOL_TIMEOUT)?;
    if bytes.is_empty() {
        None
    } else {
        Some(Thumbnail::new(bytes, Some("image/png".to_string())))
    }
}

/// #14: Generate PDF thumbnail using mutool CLI (MuPDF) if available,
/// falling back to pdftoppm (Poppler) as secondary option.
/// Renders the first page of the PDF to a PNG image.
pub fn generate_pdf_thumbnail(path: &Path, max_size: u32) -> Option<Thumbnail> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext != "pdf" {
        return None;
    }
    // Same reason as for ffmpeg: only absolute paths can never be read as options.
    if !path.is_absolute() {
        return None;
    }

    // Try mutool (MuPDF) first — outputs PNG to stdout.
    // No `--` terminator here: MuPDF's option parser is not documented to accept
    // one, and the absolute-path check above already rules out a leading '-'.
    if let Some(mutool) = resolve_tool(&MUTOOL, "mutool") {
        let args: Vec<OsString> = vec![
            OsString::from("draw"),
            OsString::from("-o"),
            OsString::from("-"), // output to stdout
            OsString::from("-F"),
            OsString::from("png"), // PNG format
            OsString::from("-w"),
            OsString::from(max_size.to_string()),
            OsString::from("-h"),
            OsString::from(max_size.to_string()),
            path.as_os_str().to_os_string(),
            OsString::from("1"), // first page only
        ];
        if let Some(bytes) = run_capturing_stdout(mutool, &args, EXTERNAL_TOOL_TIMEOUT)
            && !bytes.is_empty()
        {
            return Some(Thumbnail::new(bytes, Some("image/png".to_string())));
        }
    }

    // Fallback: pdftoppm (Poppler) — writes to a file, so it needs a workspace.
    let pdftoppm = resolve_tool(&PDFTOPPM, "pdftoppm")?;
    let workspace = TempWorkspace::create()?;
    let prefix = workspace.path().join("page");
    let args: Vec<OsString> = vec![
        OsString::from("-png"),
        OsString::from("-f"),
        OsString::from("1"),
        OsString::from("-l"),
        OsString::from("1"),
        OsString::from("-scale-to"),
        OsString::from(max_size.to_string()),
        OsString::from("--"),
        path.as_os_str().to_os_string(),
        prefix.into_os_string(),
    ];
    run_capturing_stdout(pdftoppm, &args, EXTERNAL_TOOL_TIMEOUT)?;

    // pdftoppm pads the page number to the width of the page count, so the
    // produced name is not `<prefix>-1.png` for every document. The workspace
    // belongs to this call alone, so whatever PNG is in it is ours.
    let produced = first_png_in(workspace.path())?;
    let bytes = std::fs::read(produced).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(Thumbnail::new(bytes, Some("image/png".to_string())))
}

// ── External renderer plumbing ─────────────────────────────────────────────

static FFMPEG: OnceLock<Option<PathBuf>> = OnceLock::new();
static MUTOOL: OnceLock<Option<PathBuf>> = OnceLock::new();
static PDFTOPPM: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Resolves an external renderer once per process.
///
/// Every thumbnail used to hand a bare program name to `Command`, which on a
/// machine without the tool means a full PATH walk and a failed CreateProcess
/// per file, silently. Caching the answer also lets us tell the log once that a
/// renderer is missing.
fn resolve_tool(cell: &'static OnceLock<Option<PathBuf>>, name: &str) -> Option<&'static Path> {
    cell.get_or_init(|| {
        let found = find_in_path(name);
        match &found {
            Some(path) => {
                tracing::debug!("Vignettes: {name} trouvé ({})", path.display());
            }
            None => {
                tracing::info!(
                    "Vignettes: {name} introuvable dans le PATH — les aperçus qui en dépendent sont désactivés"
                );
            }
        }
        found
    })
    .as_deref()
}

/// Looks `name` up in `PATH` and returns an absolute path.
///
/// Resolving the name ourselves keeps the current directory out of the search:
/// Windows' `CreateProcess` searches it first, so a `ffmpeg.exe` dropped into a
/// browsed folder would otherwise be the one that runs.
pub(crate) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        // An empty or relative PATH entry resolves against the current
        // directory; skip it deliberately.
        if !dir.is_absolute() {
            continue;
        }
        let direct = dir.join(name);
        if is_executable_file(&direct) {
            return Some(direct);
        }
        for extension in executable_extensions() {
            let mut file_name = OsString::from(name);
            file_name.push(&extension);
            let candidate = dir.join(file_name);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Suffixes to append to a bare program name, from `PATHEXT` on Windows.
#[cfg(windows)]
fn executable_extensions() -> Vec<OsString> {
    match std::env::var_os("PATHEXT") {
        Some(value) => std::env::split_paths(&value)
            .map(|part| part.into_os_string())
            .filter(|part| !part.is_empty())
            .collect(),
        None => vec![
            OsString::from(".EXE"),
            OsString::from(".COM"),
            OsString::from(".BAT"),
            OsString::from(".CMD"),
        ],
    }
}

#[cfg(not(windows))]
fn executable_extensions() -> Vec<OsString> {
    Vec::new()
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// Counting semaphore guarding the number of live renderer processes.
///
/// `tokio::sync::Semaphore` is not usable here: these functions are synchronous
/// and are called from inside `spawn_blocking`, where awaiting is impossible.
/// Blocking a thread of the blocking pool is exactly what that pool is for.
struct ProcessSlots {
    free: Mutex<usize>,
    released: Condvar,
}

impl ProcessSlots {
    fn acquire(&self) -> SlotGuard<'_> {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        while *free == 0 {
            free = self.released.wait(free).unwrap_or_else(|e| e.into_inner());
        }
        *free -= 1;
        SlotGuard { slots: self }
    }
}

struct SlotGuard<'a> {
    slots: &'a ProcessSlots,
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        let mut free = self.slots.free.lock().unwrap_or_else(|e| e.into_inner());
        *free += 1;
        self.slots.released.notify_one();
    }
}

fn process_slots() -> &'static ProcessSlots {
    static SLOTS: OnceLock<ProcessSlots> = OnceLock::new();
    SLOTS.get_or_init(|| ProcessSlots {
        free: Mutex::new(MAX_CONCURRENT_EXTERNAL_TOOLS),
        released: Condvar::new(),
    })
}

/// Runs `program`, returns its stdout, and kills it once `timeout` elapses.
///
/// Returns `None` for every kind of failure — spawn error, timeout, non-zero
/// exit — so callers keep the previous "no thumbnail" behaviour.
fn run_capturing_stdout(program: &Path, args: &[OsString], timeout: Duration) -> Option<Vec<u8>> {
    // RAII: the slot is held for the whole child process lifetime. Bounding this
    // is the point — without it, opening a folder of videos launched one ffmpeg
    // per file at once.
    let _slot = process_slots().acquire();

    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            tracing::debug!(
                "Vignettes: lancement de {} échoué: {error}",
                program.display()
            );
            return None;
        }
    };

    // stdout has to be drained while the child runs: a PNG frame overflows the
    // pipe buffer, and a full pipe would block the child forever below.
    let Some(stdout) = child.stdout.take() else {
        kill_and_reap(&mut child, program);
        return None;
    };
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut stdout = stdout;
        let mut buffer = Vec::new();
        if stdout.read_to_end(&mut buffer).is_err() {
            buffer.clear();
        }
        buffer
    });

    let deadline = Instant::now() + timeout;
    let mut succeeded = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                succeeded = status.success();
                break;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::debug!(
                    "Vignettes: attente de {} échouée: {error}",
                    program.display()
                );
                kill_and_reap(&mut child, program);
                break;
            }
        }
        if Instant::now() >= deadline {
            tracing::warn!(
                "Vignettes: {} dépasse {}s, processus tué",
                program.display(),
                timeout.as_secs()
            );
            kill_and_reap(&mut child, program);
            break;
        }
        std::thread::sleep(EXTERNAL_TOOL_POLL);
    }

    // The reader ends as soon as the child's stdout is closed, which killing it
    // guarantees; joining also keeps the thread from outliving this call.
    let bytes = reader.join().unwrap_or_default();
    if succeeded { Some(bytes) } else { None }
}

fn kill_and_reap(child: &mut std::process::Child, program: &Path) {
    if let Err(error) = child.kill() {
        tracing::debug!(
            "Vignettes: impossible de tuer {}: {error}",
            program.display()
        );
    }
    // Reap the child so it does not linger as a zombie.
    if let Err(error) = child.wait() {
        tracing::debug!(
            "Vignettes: attente après kill de {} échouée: {error}",
            program.display()
        );
    }
}

/// Private directory holding the output of a single pdftoppm run.
///
/// The old code wrote to a constant `xion_pdf_preview-1.png` in the shared temp
/// directory: two PDFs rendered at once overwrote each other's frame, and on
/// Unix the predictable path could be pre-created as a symlink by another user.
struct TempWorkspace {
    dir: PathBuf,
}

impl TempWorkspace {
    fn create() -> Option<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        for _ in 0..8 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let name = format!(
                "xion-pdf-{}-{}-{nanos}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let dir = base.join(name);
            match create_private_dir(&dir) {
                Ok(()) => return Some(Self { dir }),
                // Creation is atomic and fails on an existing name — including a
                // symlink planted by someone else — so retrying is the safe move.
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    tracing::debug!("Vignettes: création de {} échouée: {error}", dir.display());
                    return None;
                }
            }
        }
        None
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        // Cleanup has to happen on every path, not only after a successful read.
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            tracing::debug!(
                "Vignettes: nettoyage de {} échoué: {error}",
                self.dir.display()
            );
        }
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().create(dir)
}

fn first_png_in(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_png = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
        if is_png && path.is_file() {
            return Some(path);
        }
    }
    None
}

fn is_gif_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
}

fn is_gif_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn gif_header_detected() {
        assert!(is_gif_header(b"GIF89a"));
        assert!(is_gif_header(b"GIF87a"));
        assert!(!is_gif_header(b"\x89PNG\r\n"));
    }

    #[test]
    fn video_thumbnail_refuses_relative_path() {
        // A relative path could start with '-' and be read as an ffmpeg option.
        assert!(generate_video_thumbnail(Path::new("clip.mp4"), 128).is_none());
        assert!(generate_video_thumbnail(Path::new("-i.mp4"), 128).is_none());
    }

    #[test]
    fn pdf_thumbnail_refuses_relative_path() {
        assert!(generate_pdf_thumbnail(Path::new("doc.pdf"), 128).is_none());
        assert!(generate_pdf_thumbnail(Path::new("-o.pdf"), 128).is_none());
    }

    #[test]
    fn video_thumbnail_ignores_other_extensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("photo.png");
        assert!(generate_video_thumbnail(&path, 128).is_none());
    }

    #[test]
    fn pdf_thumbnail_ignores_other_extensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("photo.png");
        assert!(generate_pdf_thumbnail(&path, 128).is_none());
    }

    #[test]
    fn temp_workspaces_do_not_collide() {
        let a = TempWorkspace::create().expect("workspace a");
        let b = TempWorkspace::create().expect("workspace b");
        assert_ne!(a.path(), b.path());
        assert!(a.path().is_dir());
        assert!(b.path().is_dir());
        let path_a = a.path().to_path_buf();
        drop(a);
        // The directory must go away even though nothing was ever read from it.
        assert!(!path_a.exists());
    }

    #[cfg(unix)]
    #[test]
    fn temp_workspace_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let workspace = TempWorkspace::create().expect("workspace");
        let mode = std::fs::metadata(workspace.path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn first_png_ignores_other_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write");
        assert!(first_png_in(dir.path()).is_none());
        let png = dir.path().join("page-1.png");
        std::fs::write(&png, b"x").expect("write");
        assert_eq!(first_png_in(dir.path()), Some(png));
    }

    #[test]
    fn find_in_path_returns_absolute_path() {
        let Some(found) = find_in_path(if cfg!(windows) { "cmd" } else { "sh" }) else {
            return; // no shell in PATH: nothing to assert
        };
        assert!(found.is_absolute());
        assert!(found.is_file());
    }

    #[test]
    fn find_in_path_reports_missing_tool() {
        assert!(find_in_path("xion-binaire-qui-nexiste-pas").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn external_tool_is_killed_after_timeout() {
        let Some(sleep) = find_in_path("sleep") else {
            return;
        };
        let started = Instant::now();
        let result =
            run_capturing_stdout(&sleep, &[OsString::from("30")], Duration::from_millis(200));
        // Before the fix this waited the full 30 seconds and returned the output.
        assert!(result.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn external_tool_stdout_is_captured() {
        let Some(echo) = find_in_path("echo") else {
            return;
        };
        let out = run_capturing_stdout(&echo, &[OsString::from("bonjour")], Duration::from_secs(5))
            .expect("echo succeeds");
        assert_eq!(out, b"bonjour\n");
    }

    #[cfg(unix)]
    #[test]
    fn external_tool_failure_is_reported_as_none() {
        let Some(shell) = find_in_path("sh") else {
            return;
        };
        let out = run_capturing_stdout(
            &shell,
            &[OsString::from("-c"), OsString::from("exit 3")],
            Duration::from_secs(5),
        );
        assert!(out.is_none());
    }

    /// Minimal one-page PDF; enough for pdftoppm to rasterise a page.
    const TINY_PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<<>>>>endobj\n\
4 0 obj<</Length 34>>stream\n\
1 0 0 RG 10 w 20 20 m 180 180 l S\n\
endstream endobj\n\
trailer<</Size 5/Root 1 0 R>>\n\
%%EOF\n";

    #[test]
    fn video_thumbnail_renders_a_real_file() {
        if resolve_tool(&FFMPEG, "ffmpeg").is_none() {
            return; // ffmpeg not installed: nothing to exercise
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let video = dir.path().join("clip.mp4");

        // Build the input with ffmpeg itself so the test carries no binary blob.
        let generated = run_capturing_stdout(
            resolve_tool(&FFMPEG, "ffmpeg").expect("ffmpeg"),
            &[
                OsString::from("-nostdin"),
                OsString::from("-loglevel"),
                OsString::from("error"),
                OsString::from("-f"),
                OsString::from("lavfi"),
                OsString::from("-i"),
                OsString::from("testsrc=size=160x120:rate=10:duration=3"),
                OsString::from("-pix_fmt"),
                OsString::from("yuv420p"),
                video.as_os_str().to_os_string(),
            ],
            Duration::from_secs(30),
        );
        if generated.is_none() || !video.is_file() {
            return; // this ffmpeg build lacks lavfi; the real path is untestable here
        }

        let thumbnail = generate_video_thumbnail(&video, 64).expect("vignette vidéo");
        assert_eq!(thumbnail.mime.as_deref(), Some("image/png"));
        assert!(thumbnail.bytes.starts_with(b"\x89PNG"));
    }

    #[test]
    fn pdf_thumbnail_renders_a_real_file() {
        if resolve_tool(&MUTOOL, "mutool").is_none()
            && resolve_tool(&PDFTOPPM, "pdftoppm").is_none()
        {
            return; // no PDF renderer installed
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let pdf = dir.path().join("doc.pdf");
        std::fs::write(&pdf, TINY_PDF).expect("write pdf");

        let thumbnail = generate_pdf_thumbnail(&pdf, 64).expect("vignette PDF");
        assert_eq!(thumbnail.mime.as_deref(), Some("image/png"));
        assert!(thumbnail.bytes.starts_with(b"\x89PNG"));
    }

    #[test]
    fn process_slots_bound_concurrency() {
        let slots = ProcessSlots {
            free: Mutex::new(MAX_CONCURRENT_EXTERNAL_TOOLS),
            released: Condvar::new(),
        };
        let live = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..(MAX_CONCURRENT_EXTERNAL_TOOLS * 4) {
                scope.spawn(|| {
                    let _guard = slots.acquire();
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(30));
                    live.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        assert!(peak.load(Ordering::SeqCst) >= 1);
        assert!(peak.load(Ordering::SeqCst) <= MAX_CONCURRENT_EXTERNAL_TOOLS);
    }
}
