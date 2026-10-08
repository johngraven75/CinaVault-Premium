//! Transcoding for the built-in player and the embedded server.
//!
//! The decisions (quality profile, encoder choice, buffer sizes, ffmpeg
//! arguments, ffprobe parsing) are pure functions with unit tests. The few
//! impure helpers at the bottom run ffmpeg/ffprobe and read the switches.
//!
//! Output is always fragmented MP4 with H.264 video and stereo AAC audio,
//! which every WebView2/Chromium build can decode, written to stdout so it can
//! be streamed while it is produced. Seeking restarts the job with a new
//! `start_secs`.

use crate::db::Database;
use crate::feature_flags;
use serde::Serialize;
use serde_json::{Map, Value};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Settings > Performance > Quality Control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Auto,
    Max,
    Balanced,
    Low,
}

impl Quality {
    /// Reads the saved setting; anything unknown is `Auto`.
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("max") => Self::Max,
            Some("balanced") => Self::Balanced,
            Some("low") => Self::Low,
            _ => Self::Auto,
        }
    }
}

/// Bitrates and limits for one transcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityProfile {
    pub video_kbps: u32,
    pub audio_kbps: u32,
    /// Output height cap; `None` keeps the source size.
    pub max_height: Option<u32>,
    /// libx264 speed preset (hardware encoders use their own fast preset).
    pub x264_preset: &'static str,
}

/// Maps Quality Control to a profile. `Auto` follows the source resolution so
/// a 720p file is not sent at a 4K bitrate.
pub fn quality_profile(quality: Quality, source_height: Option<u32>) -> QualityProfile {
    match quality {
        Quality::Max => QualityProfile {
            video_kbps: 20_000,
            audio_kbps: 320,
            max_height: None,
            x264_preset: "fast",
        },
        Quality::Balanced => QualityProfile {
            video_kbps: 8_000,
            audio_kbps: 192,
            max_height: Some(1080),
            x264_preset: "veryfast",
        },
        Quality::Low => QualityProfile {
            video_kbps: 2_500,
            audio_kbps: 128,
            max_height: Some(720),
            x264_preset: "veryfast",
        },
        Quality::Auto => {
            let (video_kbps, max_height) = match source_height {
                Some(h) if h <= 480 => (2_000, Some(480)),
                Some(h) if h <= 720 => (4_500, Some(720)),
                Some(h) if h <= 1080 => (10_000, Some(1080)),
                Some(_) => (20_000, None),
                None => (10_000, Some(1080)),
            };
            QualityProfile {
                video_kbps,
                audio_kbps: 192,
                max_height,
                x264_preset: "veryfast",
            }
        }
    }
}

/// H.264 encoders the transcoder can use, hardware first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Encoder {
    Nvenc,
    Qsv,
    Amf,
    Libx264,
}

/// Hardware encoders in order of preference.
pub const HARDWARE_ENCODERS: [Encoder; 3] = [Encoder::Nvenc, Encoder::Qsv, Encoder::Amf];

impl Encoder {
    pub fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Amf => "h264_amf",
            Self::Libx264 => "libx264",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Nvenc => "NVENC",
            Self::Qsv => "Quick Sync",
            Self::Amf => "AMF",
            Self::Libx264 => "Software (x264)",
        }
    }
}

/// Hardware H.264 encoders named in `ffmpeg -encoders` output, in preference order.
pub fn listed_hardware_encoders(encoders_output: &str) -> Vec<Encoder> {
    let listed: Vec<&str> = encoders_output
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .collect();
    HARDWARE_ENCODERS
        .into_iter()
        .filter(|encoder| listed.contains(&encoder.ffmpeg_name()))
        .collect()
}

/// Hardware Transcoding on picks the first working GPU encoder; otherwise libx264.
pub fn choose_encoder(hardware_enabled: bool, usable_hardware: &[Encoder]) -> Encoder {
    if !hardware_enabled {
        return Encoder::Libx264;
    }
    HARDWARE_ENCODERS
        .into_iter()
        .find(|encoder| usable_hardware.contains(encoder))
        .unwrap_or(Encoder::Libx264)
}

/// Stream Buffering Control, as both sides apply it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferPlan {
    /// Size of each read the server sends to the client.
    pub chunk_bytes: usize,
    /// Seconds the player keeps buffered ahead before resuming after a stall (0 = browser default).
    pub buffer_ahead_secs: u32,
}

pub const DEFAULT_CHUNK_KB: u64 = 256;
pub const DEFAULT_BUFFER_AHEAD_SECS: u64 = 30;
const UNMANAGED_CHUNK_BYTES: usize = 64 * 1024;

fn config_u64(config: &Map<String, Value>, key: &str) -> Option<u64> {
    config.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_f64().filter(|v| *v >= 0.0).map(|v| v as u64))
            .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

/// The switch off leaves buffering to the browser and uses 64 KiB server reads.
pub fn buffer_plan(enabled: bool, config: &Map<String, Value>) -> BufferPlan {
    if !enabled {
        return BufferPlan {
            chunk_bytes: UNMANAGED_CHUNK_BYTES,
            buffer_ahead_secs: 0,
        };
    }
    let chunk_kb = config_u64(config, "chunkKb")
        .unwrap_or(DEFAULT_CHUNK_KB)
        .clamp(16, 4096);
    let ahead = config_u64(config, "bufferAheadSeconds")
        .unwrap_or(DEFAULT_BUFFER_AHEAD_SECS)
        .clamp(5, 300);
    BufferPlan {
        chunk_bytes: (chunk_kb * 1024) as usize,
        buffer_ahead_secs: ahead as u32,
    }
}

/// Everything ffmpeg needs for one transcode.
#[derive(Debug, Clone)]
pub struct TranscodeJob<'a> {
    pub input: &'a str,
    pub start_secs: f64,
    pub encoder: Encoder,
    pub profile: QualityProfile,
}

/// Seconds between forced keyframes, which is also the fragment length.
const KEYFRAME_SECS: u32 = 2;

/// ffmpeg arguments (without the program name) for a fragmented-MP4 stream on stdout.
pub fn ffmpeg_args(job: &TranscodeJob) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let start = if job.start_secs.is_finite() {
        job.start_secs.max(0.0)
    } else {
        0.0
    };
    if start > 0.0 {
        // Input seeking: fast, and the output timeline restarts at 0.
        args.extend(["-ss".into(), format!("{start:.3}")]);
    }
    args.extend(["-i".into(), job.input.to_string()]);
    args.extend(
        ["-map", "0:v:0?", "-map", "0:a:0?", "-sn", "-dn"]
            .iter()
            .map(|s| s.to_string()),
    );

    args.extend(["-c:v".into(), job.encoder.ffmpeg_name().into()]);
    let pix_fmt = match job.encoder {
        Encoder::Libx264 => {
            args.extend(["-preset".into(), job.profile.x264_preset.into()]);
            "yuv420p"
        }
        Encoder::Nvenc => {
            args.extend(["-preset".into(), "p4".into()]);
            "yuv420p"
        }
        Encoder::Qsv => {
            args.extend(["-preset".into(), "veryfast".into()]);
            "nv12"
        }
        Encoder::Amf => {
            args.extend(["-quality".into(), "speed".into()]);
            "yuv420p"
        }
    };
    if let Some(height) = job.profile.max_height {
        // Only ever scales down; -2 keeps the width even for 4:2:0.
        args.extend(["-vf".into(), format!("scale=-2:min(ih\\,{height})")]);
    }
    let kbps = job.profile.video_kbps;
    args.extend([
        "-pix_fmt".into(),
        pix_fmt.into(),
        "-profile:v".into(),
        "high".into(),
        "-b:v".into(),
        format!("{kbps}k"),
        "-maxrate".into(),
        format!("{kbps}k"),
        "-bufsize".into(),
        format!("{}k", kbps * 2),
        "-force_key_frames".into(),
        format!("expr:gte(t,n_forced*{KEYFRAME_SECS})"),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{}k", job.profile.audio_kbps),
        "-ac".into(),
        "2".into(),
        "-max_muxing_queue_size".into(),
        "1024".into(),
        "-movflags".into(),
        "frag_keyframe+empty_moov+default_base_moof".into(),
        "-f".into(),
        "mp4".into(),
        "pipe:1".into(),
    ]);
    args
}

/// One chapter marker from the file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chapter {
    pub start: f64,
    pub end: f64,
    pub title: String,
}

/// What the player needs to know about a file to decide how to play it.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub duration: f64,
    pub format_name: String,
    pub video_codec: Option<String>,
    pub video_profile: Option<String>,
    pub pix_fmt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub audio_codec: Option<String>,
    pub chapters: Vec<Chapter>,
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
        .filter(|v| v.is_finite())
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Parses `ffprobe -print_format json -show_format -show_streams -show_chapters`.
pub fn parse_probe(json: &Value) -> MediaProbe {
    let streams = json["streams"].as_array().cloned().unwrap_or_default();
    // Cover art is a "video" stream too; skip attached pictures.
    let video = streams.iter().find(|s| {
        s["codec_type"] == "video" && s["disposition"]["attached_pic"].as_i64().unwrap_or(0) == 0
    });
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let duration = number(&json["format"]["duration"])
        .or_else(|| video.and_then(|v| number(&v["duration"])))
        .unwrap_or(0.0)
        .max(0.0);
    let chapters = json["chapters"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|c| {
                    let start = number(&c["start_time"])?;
                    let end = number(&c["end_time"])?;
                    (end > start).then(|| Chapter {
                        start,
                        end,
                        title: text(&c["tags"]["title"]).unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    MediaProbe {
        duration,
        format_name: text(&json["format"]["format_name"]).unwrap_or_default(),
        video_codec: video.and_then(|v| text(&v["codec_name"])),
        video_profile: video.and_then(|v| text(&v["profile"])),
        pix_fmt: video.and_then(|v| text(&v["pix_fmt"])),
        width: video.and_then(|v| v["width"].as_u64()).map(|w| w as u32),
        height: video.and_then(|v| v["height"].as_u64()).map(|h| h as u32),
        audio_codec: audio.and_then(|a| text(&a["codec_name"])),
        chapters,
    }
}

// ── Impure helpers ───────────────────────────────────────────────────────────

fn tool_command(executable: &str) -> Command {
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut command = Command::new(crate::media_tools::resolve_executable(executable));
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Path of the ffmpeg binary media_tools manages.
pub fn ffmpeg_path() -> std::path::PathBuf {
    crate::media_tools::resolve_executable("ffmpeg")
}

/// Whether ffmpeg runs at all (cached for the life of the process once found).
pub fn ffmpeg_available() -> bool {
    static FOUND: OnceLock<()> = OnceLock::new();
    if FOUND.get().is_some() {
        return true;
    }
    let ok = tool_command("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if ok {
        let _ = FOUND.set(());
    }
    ok
}

/// Hardware encoders that are listed by `ffmpeg -encoders` and actually encode
/// a test frame here (a listed NVENC without an NVIDIA GPU fails). Probed once.
pub fn usable_hardware_encoders() -> &'static [Encoder] {
    static USABLE: OnceLock<Vec<Encoder>> = OnceLock::new();
    USABLE.get_or_init(|| {
        let listed = tool_command("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|out| listed_hardware_encoders(&String::from_utf8_lossy(&out.stdout)))
            .unwrap_or_default();
        let usable: Vec<Encoder> = listed
            .into_iter()
            .filter(|encoder| {
                tool_command("ffmpeg")
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=black:s=256x144:r=25:d=0.2",
                        "-frames:v",
                        "3",
                        "-c:v",
                        encoder.ffmpeg_name(),
                        "-f",
                        "null",
                        "-",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
            })
            .collect();
        log::info!(
            "Hardware H.264 encoders usable: {:?}",
            usable.iter().map(|e| e.ffmpeg_name()).collect::<Vec<_>>()
        );
        usable
    })
}

/// Runs ffprobe on `path`.
pub fn probe_media(path: &str) -> Result<MediaProbe, String> {
    let output = tool_command("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("ffprobe could not start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe could not read the file: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("ffprobe output was not JSON: {error}"))?;
    Ok(parse_probe(&json))
}

/// The switches and setting that shape a transcode, read once per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscodeSettings {
    pub quality: Quality,
    pub hardware: bool,
    pub buffer: BufferPlan,
}

impl TranscodeSettings {
    pub fn load(db: &Database) -> Self {
        Self {
            quality: Quality::parse(
                db.get_setting_data("quality_control")
                    .ok()
                    .flatten()
                    .as_deref(),
            ),
            hardware: feature_flags::is_enabled(db, "hw_transcode"),
            buffer: buffer_plan(
                feature_flags::is_enabled(db, "stream_buffer"),
                &feature_flags::config(db, "stream_buffer"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pos(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].clone())
    }

    #[test]
    fn quality_control_maps_to_bitrate_and_size() {
        assert_eq!(Quality::parse(Some("max")), Quality::Max);
        assert_eq!(Quality::parse(Some(" Balanced ")), Quality::Balanced);
        assert_eq!(Quality::parse(Some("low")), Quality::Low);
        assert_eq!(Quality::parse(Some("weird")), Quality::Auto);
        assert_eq!(Quality::parse(None), Quality::Auto);

        let max = quality_profile(Quality::Max, Some(2160));
        assert_eq!((max.video_kbps, max.max_height), (20_000, None));
        let low = quality_profile(Quality::Low, Some(2160));
        assert_eq!(
            (low.video_kbps, low.audio_kbps, low.max_height),
            (2_500, 128, Some(720))
        );
        let balanced = quality_profile(Quality::Balanced, None);
        assert_eq!(
            (balanced.video_kbps, balanced.max_height),
            (8_000, Some(1080))
        );

        assert_eq!(quality_profile(Quality::Auto, Some(480)).video_kbps, 2_000);
        assert_eq!(quality_profile(Quality::Auto, Some(720)).video_kbps, 4_500);
        assert_eq!(
            quality_profile(Quality::Auto, Some(1080)).video_kbps,
            10_000
        );
        assert_eq!(quality_profile(Quality::Auto, Some(2160)).max_height, None);
        assert_eq!(quality_profile(Quality::Auto, None).max_height, Some(1080));
    }

    #[test]
    fn encoder_choice_prefers_working_gpu_encoders() {
        let listing = " V....D libx264              libx264 H.264\n V....D h264_nvenc           NVIDIA NVENC H.264 encoder\n V..... h264_qsv             Intel Quick Sync\n A....D aac                  AAC";
        assert_eq!(
            listed_hardware_encoders(listing),
            vec![Encoder::Nvenc, Encoder::Qsv]
        );
        assert!(listed_hardware_encoders("V..... h264_nvenc_fake x").is_empty());
        assert_eq!(
            choose_encoder(true, &[Encoder::Qsv, Encoder::Nvenc]),
            Encoder::Nvenc
        );
        assert_eq!(choose_encoder(true, &[Encoder::Amf]), Encoder::Amf);
        assert_eq!(choose_encoder(true, &[]), Encoder::Libx264);
        assert_eq!(choose_encoder(false, &[Encoder::Nvenc]), Encoder::Libx264);
    }

    #[test]
    fn buffer_plan_follows_switch_and_clamps() {
        let off = buffer_plan(false, &Map::new());
        assert_eq!((off.chunk_bytes, off.buffer_ahead_secs), (65_536, 0));
        let default_on = buffer_plan(true, &Map::new());
        assert_eq!(
            (default_on.chunk_bytes, default_on.buffer_ahead_secs),
            (262_144, 30)
        );
        let custom = json!({ "chunkKb": 1024, "bufferAheadSeconds": "90" });
        let plan = buffer_plan(true, custom.as_object().unwrap());
        assert_eq!((plan.chunk_bytes, plan.buffer_ahead_secs), (1_048_576, 90));
        let wild = json!({ "chunkKb": 1, "bufferAheadSeconds": 10_000 });
        let plan = buffer_plan(true, wild.as_object().unwrap());
        assert_eq!((plan.chunk_bytes, plan.buffer_ahead_secs), (16_384, 300));
    }

    #[test]
    fn ffmpeg_args_build_a_seekable_fragmented_mp4() {
        let job = TranscodeJob {
            input: "C:\\Media\\Film [2024].mkv",
            start_secs: 754.5,
            encoder: Encoder::Libx264,
            profile: quality_profile(Quality::Balanced, Some(2160)),
        };
        let args = ffmpeg_args(&job);
        assert_eq!(pos(&args, "-ss").as_deref(), Some("754.500"));
        assert!(
            args.iter().position(|a| a == "-ss") < args.iter().position(|a| a == "-i"),
            "seek must come before -i"
        );
        assert_eq!(
            pos(&args, "-i").as_deref(),
            Some("C:\\Media\\Film [2024].mkv")
        );
        assert_eq!(pos(&args, "-c:v").as_deref(), Some("libx264"));
        assert_eq!(pos(&args, "-preset").as_deref(), Some("veryfast"));
        assert_eq!(pos(&args, "-b:v").as_deref(), Some("8000k"));
        assert_eq!(pos(&args, "-bufsize").as_deref(), Some("16000k"));
        assert_eq!(
            pos(&args, "-vf").as_deref(),
            Some("scale=-2:min(ih\\,1080)")
        );
        assert_eq!(pos(&args, "-c:a").as_deref(), Some("aac"));
        assert_eq!(pos(&args, "-b:a").as_deref(), Some("192k"));
        assert_eq!(
            pos(&args, "-movflags").as_deref(),
            Some("frag_keyframe+empty_moov+default_base_moof")
        );
        assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
    }

    #[test]
    fn ffmpeg_args_use_each_hardware_encoder_correctly() {
        let mut job = TranscodeJob {
            input: "in.mkv",
            start_secs: 0.0,
            encoder: Encoder::Nvenc,
            profile: quality_profile(Quality::Max, None),
        };
        let nvenc = ffmpeg_args(&job);
        assert!(!nvenc.contains(&"-ss".to_string()));
        assert!(!nvenc.contains(&"-vf".to_string()));
        assert_eq!(pos(&nvenc, "-c:v").as_deref(), Some("h264_nvenc"));
        assert_eq!(pos(&nvenc, "-preset").as_deref(), Some("p4"));
        assert_eq!(pos(&nvenc, "-b:v").as_deref(), Some("20000k"));

        job.encoder = Encoder::Qsv;
        let qsv = ffmpeg_args(&job);
        assert_eq!(pos(&qsv, "-c:v").as_deref(), Some("h264_qsv"));
        assert_eq!(pos(&qsv, "-pix_fmt").as_deref(), Some("nv12"));

        job.encoder = Encoder::Amf;
        job.start_secs = f64::NAN;
        let amf = ffmpeg_args(&job);
        assert_eq!(pos(&amf, "-c:v").as_deref(), Some("h264_amf"));
        assert_eq!(pos(&amf, "-quality").as_deref(), Some("speed"));
        assert!(!amf.contains(&"-ss".to_string()));
    }

    #[test]
    fn probe_parsing_reads_streams_and_chapters() {
        let json = json!({
            "format": { "duration": "1425.120000", "format_name": "matroska,webm" },
            "streams": [
                { "codec_type": "video", "codec_name": "mjpeg", "disposition": { "attached_pic": 1 } },
                { "codec_type": "video", "codec_name": "h264", "profile": "High 10", "pix_fmt": "yuv420p10le",
                  "width": 1920, "height": 1080, "disposition": { "attached_pic": 0 } },
                { "codec_type": "audio", "codec_name": "eac3" }
            ],
            "chapters": [
                { "start_time": "0.000000", "end_time": "88.5", "tags": { "title": "Opening" } },
                { "start_time": "88.5", "end_time": "1300", "tags": {} },
                { "start_time": "1300", "end_time": "1300" }
            ]
        });
        let probe = parse_probe(&json);
        assert_eq!(probe.duration, 1425.12);
        assert_eq!(probe.format_name, "matroska,webm");
        assert_eq!(probe.video_codec.as_deref(), Some("h264"));
        assert_eq!(probe.video_profile.as_deref(), Some("High 10"));
        assert_eq!(probe.pix_fmt.as_deref(), Some("yuv420p10le"));
        assert_eq!((probe.width, probe.height), (Some(1920), Some(1080)));
        assert_eq!(probe.audio_codec.as_deref(), Some("eac3"));
        assert_eq!(probe.chapters.len(), 2);
        assert_eq!(probe.chapters[0].title, "Opening");
        assert_eq!(probe.chapters[1].title, "");
        assert_eq!(parse_probe(&json!({})), MediaProbe::default());
    }

    #[test]
    fn settings_load_reads_switches_and_quality() {
        let path =
            std::env::temp_dir().join(format!("cinavault-transcode-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.to_str().unwrap()).unwrap();
        let defaults = TranscodeSettings::load(&db);
        assert_eq!(defaults.quality, Quality::Auto);
        assert!(defaults.hardware);
        assert_eq!(defaults.buffer.buffer_ahead_secs, 30);

        db.set_setting_data("quality_control", "low").unwrap();
        db.set_feature_setting_data("hw_transcode", false, "{}")
            .unwrap();
        db.set_feature_setting_data("stream_buffer", true, r#"{"chunkKb":512}"#)
            .unwrap();
        let saved = TranscodeSettings::load(&db);
        assert_eq!(saved.quality, Quality::Low);
        assert!(!saved.hardware);
        assert_eq!(saved.buffer.chunk_bytes, 524_288);
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
