//! End-to-end tests for the `m4againrs` CLI binary.
//!
//! These tests spawn the compiled binary via `CARGO_BIN_EXE_m4againrs`,
//! generate input fixtures with `ffmpeg`, and validate outputs with
//! `ffprobe`/`ffmpeg`. The library crate (`src/`) deliberately avoids any
//! ffmpeg dependency in its tests; ffmpeg is reserved for the CLI's
//! end-to-end coverage so the library remains testable in isolation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_m4againrs");

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX_EPOCH")
            .as_nanos();
        let path = workspace_tmp_root()
            .join("cli-tests")
            .join(format!("{name}-{}-{now}", std::process::id()));
        fs::create_dir_all(&path).expect("failed to create test directory under ./tmp");
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn workspace_tmp_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate must be inside workspace")
        .join("tmp")
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("failed to spawn m4againrs binary")
}

fn ffmpeg_generate_tone(path: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2:sample_rate=44100",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
        ])
        .arg(path)
        .status()
        .expect("failed to spawn ffmpeg — is it installed?");
    assert!(status.success(), "ffmpeg failed to generate AAC fixture");
}

fn ffmpeg_decode_check(path: &Path) {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-i",
        ])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()
        .expect("failed to spawn ffmpeg for decode check");
    assert!(
        output.status.success(),
        "ffmpeg failed to decode {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn ffprobe_m4ag_tag(path: &Path) -> String {
    let output = Command::new("ffprobe")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-show_entries",
            "format_tags=M4AG",
            "-of",
            "default=nw=1",
            "-export_all",
            "1",
        ])
        .arg(path)
        .output()
        .expect("failed to spawn ffprobe");
    assert!(
        output.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("ffprobe stdout was not valid UTF-8")
}

#[test]
fn cli_prints_usage_and_exits_2_when_args_missing() {
    let out = run_cli(&[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage: m4againrs"), "stderr was: {stderr}");
}

#[test]
fn cli_exits_2_when_too_many_args() {
    let tmp = TestDir::new("cli-too-many-args");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&src);

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "2",
        "extra",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_exits_2_when_gain_steps_not_an_integer() {
    let tmp = TestDir::new("cli-bad-steps");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&src);

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "loud"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must be an integer"),
        "stderr was: {stderr}"
    );
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_fails_on_zero_gain_steps() {
    let tmp = TestDir::new("cli-zero-gain");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&src);

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "0"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("gain_steps"), "stderr was: {stderr}");
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_fails_when_source_equals_destination() {
    let tmp = TestDir::new("cli-same-path");
    let src = tmp.join("in.m4a");
    ffmpeg_generate_tone(&src);
    let before = fs::read(&src).expect("failed to read fixture");

    let out = run_cli(&[src.to_str().unwrap(), src.to_str().unwrap(), "2"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        fs::read(&src).expect("failed to reread source"),
        before,
        "source must be untouched"
    );
}

#[test]
fn cli_fails_on_nonexistent_input() {
    let tmp = TestDir::new("cli-missing-input");
    let src = tmp.join("does-not-exist.m4a");
    let dst = tmp.join("out.m4a");

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "2"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_fails_on_non_mp4_input() {
    let tmp = TestDir::new("cli-not-mp4");
    let src = tmp.join("input.bin");
    let dst = tmp.join("out.m4a");
    fs::write(&src, b"not an mp4 at all").expect("failed to write garbage fixture");

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "2"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_writes_destination_file_with_gain_metadata_and_ffmpeg_decodes() {
    let tmp = TestDir::new("cli-file-positive");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&src);
    let src_before = fs::read(&src).expect("failed to read source");

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "2"]);
    assert!(
        out.status.success(),
        "cli failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(dst.exists(), "destination file should exist");
    assert_eq!(
        fs::read(&src).expect("failed to reread source"),
        src_before,
        "cli must not modify source"
    );

    let tag = ffprobe_m4ag_tag(&dst);
    assert!(
        tag.contains("TAG:M4AG=m4againrs version=1 gain_steps=2 gain_step_db=1.5"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&dst);
}

#[test]
fn cli_negative_gain_writes_destination_with_metadata() {
    let tmp = TestDir::new("cli-file-negative");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&src);

    let out = run_cli(&[src.to_str().unwrap(), dst.to_str().unwrap(), "-3"]);
    assert!(
        out.status.success(),
        "cli failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tag = ffprobe_m4ag_tag(&dst);
    assert!(
        tag.contains("gain_steps=-3"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&dst);
}

#[test]
fn cli_streams_to_stdout_when_destination_is_dash() {
    let tmp = TestDir::new("cli-stdout");
    let src = tmp.join("in.m4a");
    let captured = tmp.join("from-stdout.m4a");
    ffmpeg_generate_tone(&src);

    let out = run_cli(&[src.to_str().unwrap(), "-", "4"]);
    assert!(
        out.status.success(),
        "cli failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.stdout.is_empty(), "stdout was empty");

    fs::write(&captured, &out.stdout).expect("failed to write captured stdout");
    let tag = ffprobe_m4ag_tag(&captured);
    assert!(
        tag.contains("gain_steps=4"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&captured);
}

#[test]
fn cli_stdout_output_round_trips_through_pipe() {
    let tmp = TestDir::new("cli-stdout-pipe");
    let src = tmp.join("in.m4a");
    ffmpeg_generate_tone(&src);

    let mut child = Command::new(BIN)
        .args([src.to_str().unwrap(), "-", "2"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn m4againrs");
    let stdout = child.stdout.take().expect("missing piped stdout");

    let ffmpeg = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-i",
            "pipe:0",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::from(stdout))
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ffmpeg");

    let cli_status = child.wait().expect("cli wait failed");
    let ffmpeg_out = ffmpeg.wait_with_output().expect("ffmpeg wait failed");

    assert!(cli_status.success(), "cli exited non-zero");
    assert!(
        ffmpeg_out.status.success(),
        "ffmpeg failed to decode piped stream: {}",
        String::from_utf8_lossy(&ffmpeg_out.stderr)
    );
}

#[test]
fn cli_round_trip_zero_steps_after_combining_positive_and_negative_decodes() {
    let tmp = TestDir::new("cli-roundtrip");
    let src = tmp.join("in.m4a");
    let louder = tmp.join("louder.m4a");
    let restored = tmp.join("restored.m4a");
    ffmpeg_generate_tone(&src);

    let up = run_cli(&[src.to_str().unwrap(), louder.to_str().unwrap(), "5"]);
    assert!(up.status.success());
    let down = run_cli(&[louder.to_str().unwrap(), restored.to_str().unwrap(), "-5"]);
    assert!(down.status.success());

    ffmpeg_decode_check(&restored);
    let tag = ffprobe_m4ag_tag(&restored);
    assert!(
        tag.contains("gain_steps=-5"),
        "expected most-recent gain step to be recorded: {tag}"
    );
}

