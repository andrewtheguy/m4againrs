//! End-to-end tests for the `m4againrs` CLI binary.
//!
//! These tests spawn the compiled binary via `CARGO_BIN_EXE_m4againrs`,
//! generate input fixtures with `ffmpeg`, and validate outputs with
//! `ffprobe`/`ffmpeg`. The library crate (`src/`) deliberately avoids any
//! ffmpeg dependency in its tests; ffmpeg is reserved for the CLI's
//! end-to-end coverage so the library remains testable in isolation.

use std::fs;
use std::io::{Cursor, Write};
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

fn ffmpeg_remux_faststart(src: &Path, dst: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-i",
        ])
        .arg(src)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(dst)
        .status()
        .expect("failed to spawn ffmpeg for faststart remux");
    assert!(status.success(), "ffmpeg failed to remux to faststart");
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
        "extra",
        "--gain-steps",
        "2",
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "loud",
    ]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid value") && stderr.contains("--gain-steps"),
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "0",
    ]);
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        src.to_str().unwrap(),
        "--gain-steps",
        "2",
    ]);
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "2",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!dst.exists(), "destination should not be created");
}

#[test]
fn cli_fails_on_non_mp4_input() {
    let tmp = TestDir::new("cli-not-mp4");
    let src = tmp.join("input.bin");
    let dst = tmp.join("out.m4a");
    fs::write(&src, b"not an mp4 at all").expect("failed to write garbage fixture");

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "2",
    ]);
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "2",
    ]);
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

    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "-3",
    ]);
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

    let out = run_cli(&[src.to_str().unwrap(), "-", "--gain-steps", "4"]);
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
        .args([src.to_str().unwrap(), "-", "--gain-steps", "2"])
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

    let up = run_cli(&[
        src.to_str().unwrap(),
        louder.to_str().unwrap(),
        "--gain-steps",
        "5",
    ]);
    assert!(up.status.success());
    let down = run_cli(&[
        louder.to_str().unwrap(),
        restored.to_str().unwrap(),
        "--gain-steps",
        "-5",
    ]);
    assert!(down.status.success());

    ffmpeg_decode_check(&restored);
    let tag = ffprobe_m4ag_tag(&restored);
    assert!(
        tag.contains("gain_steps=-5"),
        "expected most-recent gain step to be recorded: {tag}"
    );
}

#[test]
fn cli_streaming_api_matches_writer_api_byte_for_byte() {
    // Generate a fresh AAC tone with ffmpeg, remux to faststart, then assert
    // the streaming API and the seekable writer API produce byte-identical
    // output for the same input. Also pipe the streaming output through
    // ffmpeg to confirm it decodes cleanly.
    let tmp = TestDir::new("cli-streaming-equality");
    let raw = tmp.join("raw.m4a");
    let faststart = tmp.join("faststart.m4a");
    let streaming_out = tmp.join("streaming.m4a");
    ffmpeg_generate_tone(&raw);
    ffmpeg_remux_faststart(&raw, &faststart);

    let bytes = fs::read(&faststart).expect("failed to read faststart fixture");

    let mut writer_out = Vec::new();
    let writer_modified =
        m4againrs::aac_apply_gain_to_writer(&mut Cursor::new(&bytes), &mut writer_out, 4)
            .expect("writer API should accept faststart input");

    let mut stream_out = Vec::new();
    let stream_modified =
        m4againrs::aac_apply_gain_streaming(&mut Cursor::new(&bytes), &mut stream_out, 4)
            .expect("streaming API should accept faststart input");

    assert!(stream_modified > 0);
    assert_eq!(writer_modified, stream_modified);
    assert_eq!(writer_out, stream_out, "streaming output must equal writer output byte-for-byte");

    fs::write(&streaming_out, &stream_out).expect("failed to write streaming output");
    ffmpeg_decode_check(&streaming_out);
}

#[test]
fn cli_stdin_input_to_file_output_writes_decodable_m4a() {
    let tmp = TestDir::new("cli-stdin-file");
    let raw = tmp.join("raw.m4a");
    let faststart = tmp.join("faststart.m4a");
    let dst = tmp.join("out.m4a");
    ffmpeg_generate_tone(&raw);
    ffmpeg_remux_faststart(&raw, &faststart);
    let bytes = fs::read(&faststart).expect("failed to read faststart fixture");

    let mut child = Command::new(BIN)
        .args(["-", dst.to_str().unwrap(), "--gain-steps", "3"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn m4againrs");
    child
        .stdin
        .take()
        .expect("missing piped stdin")
        .write_all(&bytes)
        .expect("failed to write to cli stdin");
    let out = child.wait_with_output().expect("cli wait failed");
    assert!(
        out.status.success(),
        "cli exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tag = ffprobe_m4ag_tag(&dst);
    assert!(
        tag.contains("gain_steps=3"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&dst);
}

#[test]
fn cli_stdin_to_stdout_round_trips_through_pipe() {
    let tmp = TestDir::new("cli-stdin-stdout");
    let raw = tmp.join("raw.m4a");
    let faststart = tmp.join("faststart.m4a");
    ffmpeg_generate_tone(&raw);
    ffmpeg_remux_faststart(&raw, &faststart);
    let bytes = fs::read(&faststart).expect("failed to read faststart fixture");

    let mut child = Command::new(BIN)
        .args(["-", "-", "--gain-steps", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn m4againrs");
    child
        .stdin
        .take()
        .expect("missing piped stdin")
        .write_all(&bytes)
        .expect("failed to write to cli stdin");
    let out = child.wait_with_output().expect("cli wait failed");
    assert!(
        out.status.success(),
        "cli exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.stdout.is_empty(), "cli stdout was empty");

    let captured = tmp.join("captured.m4a");
    fs::write(&captured, &out.stdout).expect("failed to write captured stdout");
    let tag = ffprobe_m4ag_tag(&captured);
    assert!(
        tag.contains("gain_steps=2"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&captured);
}

#[test]
fn cli_stdin_input_rejects_non_faststart() {
    let tmp = TestDir::new("cli-stdin-non-faststart");
    let src = tmp.join("non_faststart.m4a");
    ffmpeg_generate_tone(&src); // ffmpeg's default is non-faststart
    let bytes = fs::read(&src).expect("failed to read fixture");

    let mut child = Command::new(BIN)
        .args(["-", "-", "--gain-steps", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn m4againrs");
    let _ = child
        .stdin
        .take()
        .expect("missing piped stdin")
        .write_all(&bytes); // ignore broken-pipe if cli exits early
    let out = child.wait_with_output().expect("cli wait failed");

    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not faststart"),
        "stderr should mention faststart: {stderr}"
    );
}

#[test]
fn cli_streaming_api_rejects_non_faststart_input() {
    // ffmpeg's default AAC writer produces non-faststart output (mdat then
    // moov). Confirm the streaming API rejects it cleanly.
    let tmp = TestDir::new("cli-streaming-non-faststart");
    let src = tmp.join("non_faststart.m4a");
    ffmpeg_generate_tone(&src);

    let bytes = fs::read(&src).expect("failed to read non-faststart fixture");
    let mut dst = Vec::new();
    let err = m4againrs::aac_apply_gain_streaming(&mut Cursor::new(&bytes), &mut dst, 2)
        .expect_err("streaming API should reject non-faststart input");

    assert!(matches!(err, m4againrs::Error::NonFaststartInput));
}

