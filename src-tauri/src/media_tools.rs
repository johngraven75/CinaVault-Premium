// CinaVault Premium — startup bootstrap and execution bridge for media tools.
//
// This module performs real executable checks and, on Windows, silently asks
// winget to install missing permanent tools. It never marks a tool ready based
// only on catalog flags.
//
// Every tool runs hidden, with no stdin, and under a deadline. A tool that
// opens a window or waits for input (for example the MediaInfo desktop app,
// which shares the CLI's `MediaInfo.exe` name) is killed instead of freezing
// the app, and GUI-subsystem executables are never picked in the first place.
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Read;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A `--version` probe that has not answered in this long is treated as
/// missing rather than allowed to block startup.
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
/// MediaInfo and mkvmerge read container headers only; two minutes covers a
/// slow network share without letting a stuck process hold the UI forever.
const INSPECTION_TIMEOUT: Duration = Duration::from_secs(120);
/// winget downloads packages, so it gets a much longer budget.
#[cfg(target_os = "windows")]
const WINGET_TIMEOUT: Duration = Duration::from_secs(600);

/// Serialises `ensure_media_tools` so the startup repair and a UI-triggered
/// repair never run two winget installs of the same package at once, and
/// makes status reads wait for a repair in progress.
static ENSURE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy)]
struct MediaTool {
    id: &'static str,
    executable: &'static str,
    version_arg: &'static str,
    winget_package: &'static str,
}

const REQUIRED_MEDIA_TOOLS: &[MediaTool] = &[
    MediaTool {
        id: "ffmpeg",
        executable: "ffmpeg",
        version_arg: "-version",
        winget_package: "Gyan.FFmpeg",
    },
    MediaTool {
        id: "ffprobe",
        executable: "ffprobe",
        version_arg: "-version",
        winget_package: "Gyan.FFmpeg",
    },
    MediaTool {
        id: "yt-dlp",
        executable: "yt-dlp",
        version_arg: "--version",
        winget_package: "yt-dlp.yt-dlp",
    },
    MediaTool {
        id: "mediainfo",
        executable: "mediainfo",
        version_arg: "--Version",
        winget_package: "MediaArea.MediaInfo.CLI",
    },
    MediaTool {
        id: "mkvtoolnix",
        executable: "mkvmerge",
        version_arg: "--version",
        winget_package: "MoritzBunkus.MKVToolNix",
    },
];

#[derive(Debug, Serialize)]
struct ToolStatus {
    id: String,
    installed: bool,
    version: Option<String>,
    auto_install: bool,
    package: String,
}

fn executable_candidates(executable: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("PATH") {
        for directory in env::split_paths(&path) {
            let candidate = directory.join(executable);
            candidates.push(candidate.clone());
            #[cfg(target_os = "windows")]
            if candidate.extension().is_none() {
                candidates.push(directory.join(format!("{executable}.exe")));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let program_files = env::var_os("ProgramFiles").map(PathBuf::from);
        let program_files_x86 = env::var_os("ProgramFiles(x86)").map(PathBuf::from);
        let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);

        match executable {
            "mediainfo" => {
                // `Program Files\MediaInfo\MediaInfo.exe` is the desktop
                // app, not the CLI, so it is deliberately not listed here.
                for root in [program_files.clone(), program_files_x86.clone()]
                    .into_iter()
                    .flatten()
                {
                    candidates.push(root.join("MediaInfo").join("CLI").join("MediaInfo.exe"));
                }
                // winget installs MediaArea.MediaInfo.CLI as a portable
                // package under its own folder.
                if let Some(packages) = local_app_data
                    .as_ref()
                    .map(|root| root.join("Microsoft").join("WinGet").join("Packages"))
                {
                    if let Ok(entries) = std::fs::read_dir(&packages) {
                        for entry in entries.flatten() {
                            let name = entry.file_name();
                            if name
                                .to_string_lossy()
                                .starts_with("MediaArea.MediaInfo.CLI")
                            {
                                candidates.push(entry.path().join("MediaInfo.exe"));
                            }
                        }
                    }
                }
            }
            "mkvmerge" => {
                for root in [program_files.clone(), program_files_x86.clone()]
                    .into_iter()
                    .flatten()
                {
                    candidates.push(root.join("MKVToolNix").join("mkvmerge.exe"));
                }
            }
            "ffmpeg" | "ffprobe" => {
                for root in [program_files, program_files_x86, local_app_data]
                    .into_iter()
                    .flatten()
                {
                    candidates.push(root.join("ffmpeg").join(format!("{executable}.exe")));
                }
            }
            "yt-dlp" => {
                if let Some(root) = local_app_data {
                    candidates.push(root.join("Programs").join("yt-dlp").join("yt-dlp.exe"));
                }
            }
            _ => {}
        }

        // winget's shim folder. It is added to the user PATH on install, but
        // a process started before the install never sees that PATH change.
        if let Some(root) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            candidates.push(
                root.join("Microsoft")
                    .join("WinGet")
                    .join("Links")
                    .join(format!("{executable}.exe")),
            );
        }
    }

    candidates
}

/// True when `header` is a Windows PE image built for the GUI subsystem.
/// Such a program opens a window instead of printing to stdout, so it can
/// never serve as a command-line media tool.
fn is_windows_gui_image(header: &[u8]) -> bool {
    const IMAGE_SUBSYSTEM_WINDOWS_GUI: u16 = 2;
    if header.len() < 0x40 || &header[..2] != b"MZ" {
        return false;
    }
    let pe_offset =
        u32::from_le_bytes([header[0x3C], header[0x3D], header[0x3E], header[0x3F]]) as usize;
    // Signature (4) + COFF header (20) + 68 bytes into the optional header,
    // which is the same for PE32 and PE32+.
    let subsystem_offset = pe_offset.saturating_add(4 + 20 + 68);
    if header.len() < subsystem_offset + 2 || &header[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return false;
    }
    u16::from_le_bytes([header[subsystem_offset], header[subsystem_offset + 1]])
        == IMAGE_SUBSYSTEM_WINDOWS_GUI
}

fn is_usable_cli_executable(candidate: &Path) -> bool {
    if !candidate.is_file() {
        return false;
    }
    let mut header = Vec::with_capacity(4096);
    match File::open(candidate).and_then(|file| file.take(4096).read_to_end(&mut header)) {
        Ok(_) => !is_windows_gui_image(&header),
        // An unreadable file cannot be inspected; let the spawn decide.
        Err(_) => true,
    }
}

pub(crate) fn resolve_executable(executable: &str) -> PathBuf {
    locate_cli_executable(executable).unwrap_or_else(|| PathBuf::from(executable))
}

/// The executable to run for `executable`, or `None` when the only matches
/// on this machine are GUI programs. Falling back to the bare name in that
/// case would let the OS's own PATH search launch the same GUI program.
fn locate_cli_executable(executable: &str) -> Option<PathBuf> {
    let requested = Path::new(executable);
    if requested.is_absolute() && requested.is_file() {
        return Some(requested.to_path_buf());
    }

    pick_cli_candidate(executable, executable_candidates(executable))
}

fn pick_cli_candidate(
    executable: &str,
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let mut rejected_gui = false;
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        if is_usable_cli_executable(&candidate) {
            return Some(candidate);
        }
        rejected_gui = true;
    }
    (!rejected_gui).then(|| PathBuf::from(executable))
}

fn command_for(executable: &str) -> std::io::Result<Command> {
    if locate_cli_executable(executable).is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("only a desktop (GUI) version of {executable} is installed"),
        ));
    }
    let mut command = Command::new(resolve_executable(executable));
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

/// Runs `command` with no stdin and captured output, killing it if it has
/// not exited within `timeout`. Output is drained on separate threads so a
/// chatty tool cannot deadlock on a full pipe while we wait.
fn output_with_timeout(mut command: Command, timeout: Duration) -> std::io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Each drain reports through a channel so collecting output stays under
    // the same deadline: a helper the tool spawned can keep a pipe open after
    // the tool itself exits.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut buffer);
            }
            let _ = sender.send(buffer);
        });
        receiver
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("did not finish within {} seconds", timeout.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    };

    let collect = |receiver: std::sync::mpsc::Receiver<Vec<u8>>| {
        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    };
    match (collect(stdout), collect(stderr)) {
        (Ok(stdout), Ok(stderr)) => Ok(Output {
            status,
            stdout,
            stderr,
        }),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!(
                "exited but kept its output open past {} seconds",
                timeout.as_secs()
            ),
        )),
    }
}

fn executable_status(tool: MediaTool) -> ToolStatus {
    let output = command_for(tool.executable).and_then(|mut command| {
        command.arg(tool.version_arg);
        output_with_timeout(command, VERSION_PROBE_TIMEOUT)
    });
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let version = stdout
                .lines()
                .chain(stderr.lines())
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string());
            ToolStatus {
                id: tool.id.to_string(),
                installed: true,
                version,
                auto_install: true,
                package: tool.winget_package.to_string(),
            }
        }
        _ => ToolStatus {
            id: tool.id.to_string(),
            installed: false,
            version: None,
            auto_install: true,
            package: tool.winget_package.to_string(),
        },
    }
}

/// Checks all required tools concurrently instead of one at a time. Each
/// check spawns a subprocess and waits for it to exit (`Command::output`),
/// so running the 5 checks sequentially means paying for 5 process-spawn
/// round trips back to back — the dominant cost in the whole startup path
/// on a cold Windows boot. A scoped thread per tool collapses that to the
/// slowest single check instead of the sum of all five.
fn current_statuses() -> Vec<ToolStatus> {
    std::thread::scope(|scope| {
        REQUIRED_MEDIA_TOOLS
            .iter()
            .copied()
            .map(|tool| scope.spawn(move || executable_status(tool)))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or(ToolStatus {
                    id: "unknown".to_string(),
                    installed: false,
                    version: None,
                    auto_install: false,
                    package: String::new(),
                })
            })
            .collect()
    })
}

fn validate_media_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("A media file path is required.".to_string());
    }
    let candidate = Path::new(trimmed);
    if !candidate.exists() {
        return Err(format!("Media file does not exist: {trimmed}"));
    }
    if !candidate.is_file() {
        return Err(format!("Media path is not a file: {trimmed}"));
    }
    candidate
        .canonicalize()
        .map_err(|error| format!("Unable to resolve media file path: {error}"))
}

fn run_json_tool(executable: &str, args: &[&str], path: &str) -> Result<serde_json::Value, String> {
    let media_path = validate_media_path(path)?;
    let output = command_for(executable)
        .and_then(|mut command| {
            command.args(args).arg(&media_path);
            output_with_timeout(command, INSPECTION_TIMEOUT)
        })
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("{executable} is not installed. Use Recheck & Repair to install it.")
            } else {
                format!("{executable} could not inspect this file: {error}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("{executable} exited with code {:?}.", output.status.code())
        } else {
            format!("{executable} failed: {stderr}")
        });
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("{executable} returned non-UTF-8 output: {error}"))?;
    serde_json::from_str(&stdout)
        .map_err(|error| format!("{executable} returned invalid JSON: {error}"))
}

#[cfg(target_os = "windows")]
fn install_winget_package(package: &str) -> serde_json::Value {
    let mut command = match command_for("winget") {
        Ok(command) => command,
        Err(error) => {
            return serde_json::json!({
                "package": package,
                "success": false,
                "error": error.to_string(),
            })
        }
    };
    command.args([
        "install",
        "--id",
        package,
        "--exact",
        "--silent",
        "--disable-interactivity",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    match output_with_timeout(command, WINGET_TIMEOUT) {
        Ok(output) => serde_json::json!({
            "package": package,
            "success": output.status.success(),
            "exit_code": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout).trim(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim(),
        }),
        Err(error) => serde_json::json!({
            "package": package,
            "success": false,
            "error": error.to_string(),
        }),
    }
}

// Tauri runs synchronous commands on the main thread, so every command here
// is async and does its process work on the blocking pool. A slow or stuck
// tool must never freeze the window.
async fn off_main_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("Media tool task failed: {error}"))?
}

#[tauri::command]
pub async fn get_media_tools_status() -> Result<serde_json::Value, String> {
    off_main_thread(|| Ok(media_tools_status())).await
}

fn media_tools_status() -> serde_json::Value {
    // Wait for any repair in progress (the startup one included) so the UI
    // reports the post-install state rather than tools winget is still adding.
    let _guard = ENSURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tools = current_statuses();
    serde_json::json!({
        "ready": tools.iter().all(|tool| tool.installed),
        "tools": tools,
    })
}

#[tauri::command]
pub async fn ensure_media_tools() -> Result<serde_json::Value, String> {
    off_main_thread(ensure_media_tools_blocking).await
}

pub fn ensure_media_tools_blocking() -> Result<serde_json::Value, String> {
    let _guard = ENSURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = current_statuses();

    #[cfg(target_os = "windows")]
    let installations = {
        let mut attempted = HashSet::new();
        let mut results = Vec::new();
        for (tool, status) in REQUIRED_MEDIA_TOOLS.iter().zip(before.iter()) {
            if !status.installed && attempted.insert(tool.winget_package) {
                results.push(install_winget_package(tool.winget_package));
            }
        }
        results
    };

    #[cfg(not(target_os = "windows"))]
    let installations: Vec<serde_json::Value> = Vec::new();

    let after = current_statuses();
    let ready = after.iter().all(|tool| tool.installed);
    Ok(serde_json::json!({
        "type": "media_tools_startup",
        "status": if ready { "ready" } else { "missing_tools" },
        "ready": ready,
        "automatic": true,
        "authorization_prompt_required": false,
        "before": before,
        "installations": installations,
        "tools": after,
    }))
}

#[tauri::command]
pub async fn inspect_with_mediainfo(path: String) -> Result<serde_json::Value, String> {
    off_main_thread(move || run_json_tool("mediainfo", &["--Output=JSON"], &path)).await
}

#[tauri::command]
pub async fn inspect_with_mkvtoolnix(path: String) -> Result<serde_json::Value, String> {
    off_main_thread(move || {
        run_json_tool(
            "mkvmerge",
            &["--identification-format", "json", "--identify"],
            &path,
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        is_windows_gui_image, media_tools_status, output_with_timeout, pick_cli_candidate,
        ENSURE_LOCK, REQUIRED_MEDIA_TOOLS,
    };
    use std::collections::HashSet;
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// Minimal PE header: MZ stub pointing at a PE signature, with the
    /// subsystem field set as requested.
    fn pe_header(subsystem: u16) -> Vec<u8> {
        let pe_offset = 0x80usize;
        let mut image = vec![0u8; pe_offset + 4 + 20 + 96];
        image[..2].copy_from_slice(b"MZ");
        image[0x3C..0x40].copy_from_slice(&(pe_offset as u32).to_le_bytes());
        image[pe_offset..pe_offset + 4].copy_from_slice(b"PE\0\0");
        let field = pe_offset + 4 + 20 + 68;
        image[field..field + 2].copy_from_slice(&subsystem.to_le_bytes());
        image
    }

    #[test]
    fn gui_executables_are_rejected_and_console_ones_kept() {
        assert!(is_windows_gui_image(&pe_header(2)));
        assert!(!is_windows_gui_image(&pe_header(3)));
        assert!(!is_windows_gui_image(
            b"\x7fELF not a pe image at all, padded to 64 bytes......."
        ));
        assert!(!is_windows_gui_image(b"MZ"));
        let mut bad_offset = pe_header(2);
        bad_offset[0x3C..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!is_windows_gui_image(&bad_offset));
    }

    #[cfg(unix)]
    #[test]
    fn a_hung_tool_is_killed_at_the_deadline() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let started = Instant::now();
        let error = output_with_timeout(command, Duration::from_millis(300)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn a_tool_waiting_on_stdin_gets_eof_instead_of_hanging() {
        let mut command = Command::new("cat");
        let output = output_with_timeout(
            {
                command.arg("-");
                command
            },
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn large_output_is_captured_without_deadlocking() {
        let mut command = Command::new("sh");
        command.args(["-c", "head -c 1000000 /dev/zero"]);
        let output = output_with_timeout(command, Duration::from_secs(10)).unwrap();
        assert_eq!(output.stdout.len(), 1_000_000);
    }

    #[test]
    fn status_waits_for_a_repair_in_progress() {
        let guard = ENSURE_LOCK.lock().unwrap();
        let reader = std::thread::spawn(media_tools_status);
        std::thread::sleep(Duration::from_millis(200));
        assert!(!reader.is_finished(), "status must not report mid-repair");
        drop(guard);
        let status = reader.join().unwrap();
        assert!(status
            .get("tools")
            .and_then(|tools| tools.as_array())
            .is_some());
    }

    #[cfg(unix)]
    #[test]
    fn output_collection_honours_the_deadline_after_the_tool_exits() {
        // The shell exits at once but leaves a background child holding
        // stdout open; collection must still stop at the deadline.
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30 & exit 0"]);
        let started = Instant::now();
        let error = output_with_timeout(command, Duration::from_millis(500)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn only_gui_matches_are_reported_missing_not_launched() {
        let dir = std::env::temp_dir().join(format!("cv-gui-only-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gui = dir.join("MediaInfo.exe");
        let cli = dir.join("mediainfo-cli.exe");
        std::fs::write(&gui, pe_header(2)).unwrap();
        std::fs::write(&cli, pe_header(3)).unwrap();
        let missing = dir.join("absent.exe");

        // Only a GUI match: refuse rather than fall back to a PATH search.
        assert_eq!(
            pick_cli_candidate("mediainfo", [missing.clone(), gui.clone()]),
            None
        );
        // A console build later in the list wins over the GUI one.
        assert_eq!(
            pick_cli_candidate("mediainfo", [gui.clone(), cli.clone()]),
            Some(cli.clone())
        );
        // Nothing on disk: the bare name is still tried, so spawn reports NotFound.
        assert_eq!(
            pick_cli_candidate("mediainfo", [missing]),
            Some(std::path::PathBuf::from("mediainfo"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_tool_reports_not_found_instead_of_panicking() {
        let error = output_with_timeout(
            Command::new("cinavault-no-such-media-tool"),
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn every_permanent_download_tool_has_an_automatic_package() {
        let ids = REQUIRED_MEDIA_TOOLS
            .iter()
            .map(|tool| tool.id)
            .collect::<HashSet<_>>();
        for required in ["ffmpeg", "ffprobe", "yt-dlp", "mediainfo", "mkvtoolnix"] {
            assert!(ids.contains(required));
        }
        assert!(REQUIRED_MEDIA_TOOLS
            .iter()
            .all(|tool| !tool.winget_package.is_empty() && !tool.executable.is_empty()));
    }
}
