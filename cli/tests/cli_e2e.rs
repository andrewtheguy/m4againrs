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

fn testdata_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate must be inside workspace")
        .join("testdata")
        .join(name)
}

fn cli_apply_gain_to_fixture(test_name: &str, fixture: &str, gain_steps: i32) {
    let tmp = TestDir::new(test_name);
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    fs::copy(testdata_path(fixture), &src).expect("failed to copy committed fixture");

    let steps = gain_steps.to_string();
    let out = run_cli(&[
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        &steps,
    ]);
    assert!(
        out.status.success(),
        "cli failed on {fixture}: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tag = ffprobe_m4ag_tag(&dst);
    let expected = format!("gain_steps={gain_steps}");
    assert!(
        tag.contains(&expected),
        "ffprobe output missing expected M4AG tag for {fixture}: {tag}"
    );
    ffmpeg_decode_check(&dst);
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

fn ffprobe_format_tag(path: &Path, key: &str) -> String {
    let output = Command::new("ffprobe")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-show_entries",
            &format!("format_tags={key}"),
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
fn cli_decodes_he_aacv2_fixture_after_gain() {
    cli_apply_gain_to_fixture("cli-fixture-he-aacv2", "he_aacv2.m4a", 2);
}

#[test]
fn cli_decodes_aac_lc_51_fixture_after_gain() {
    cli_apply_gain_to_fixture("cli-fixture-aac-lc-51", "aac_lc_51.m4a", 2);
}

#[test]
fn cli_decodes_short_window_fixture_after_gain() {
    cli_apply_gain_to_fixture("cli-fixture-aac-lc-transient", "aac_lc_transient.m4a", -2);
}

#[test]
fn cli_decodes_he_aac_v1_implicit_fixture_after_gain() {
    cli_apply_gain_to_fixture("cli-fixture-he-aac-v1", "bear_he_aac_v1.m4a", 2);
}

#[test]
fn cli_decodes_he_aac_v2_implicit_fixture_after_gain() {
    cli_apply_gain_to_fixture(
        "cli-fixture-he-aac-v2-implicit",
        "bear_he_aac_v2_implicit.m4a",
        2,
    );
}

#[test]
fn cli_decodes_aac_main_fixture_after_gain() {
    cli_apply_gain_to_fixture("cli-fixture-aac-main", "bear_aac_main.m4a", 2);
}

#[test]
fn cli_streaming_decodes_he_aacv2_faststart_after_gain() {
    let tmp = TestDir::new("cli-stream-he-aacv2-faststart");
    let captured = tmp.join("captured.m4a");
    let bytes = fs::read(testdata_path("he_aacv2_faststart.m4a"))
        .expect("failed to read HE-AACv2 faststart fixture");

    let mut child = Command::new(BIN)
        .args(["-", "-", "--gain-steps", "3"])
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

    fs::write(&captured, &out.stdout).expect("failed to write captured stdout");
    let tag = ffprobe_m4ag_tag(&captured);
    assert!(
        tag.contains("gain_steps=3"),
        "ffprobe output missing expected M4AG tag: {tag}"
    );
    ffmpeg_decode_check(&captured);
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

// ---------------------------------------------------------------------------
// Tests below were originally written against the Python binding (because the
// CLI crate didn't exist when they were authored). They exercise pure Rust
// library logic (chunk-offset rewriting, foreign iTunes-tag preservation,
// short-window stereo gain patching) and have nothing Python-specific to say,
// so they live here now. The Python suite retains only tests that actually
// exercise the binding (file-like protocol, BytesIO, RuntimeError mapping).
// ---------------------------------------------------------------------------

#[test]
fn cli_preserves_foreign_description_tag_added_by_ffmpeg() {
    let tmp = TestDir::new("cli-preserves-description");
    let raw = tmp.join("raw.m4a");
    let described = tmp.join("described.m4a");
    let dst = tmp.join("described_out.m4a");
    ffmpeg_generate_tone(&raw);

    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-i",
        ])
        .arg(&raw)
        .args([
            "-c",
            "copy",
            "-metadata",
            "description=Existing description",
        ])
        .arg(&described)
        .status()
        .expect("failed to spawn ffmpeg to add description tag");
    assert!(status.success(), "ffmpeg failed to add description tag");

    let before = ffprobe_format_tag(&described, "description");
    assert!(
        before.contains("TAG:description=Existing description"),
        "ffprobe did not see the description tag we just added: {before}"
    );

    let out = run_cli(&[
        described.to_str().unwrap(),
        dst.to_str().unwrap(),
        "--gain-steps",
        "2",
    ]);
    assert!(
        out.status.success(),
        "cli failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = ffprobe_format_tag(&dst, "description");
    assert!(
        after.contains("TAG:description=Existing description"),
        "description tag was lost after gain rewrite: {after}"
    );
    let m4ag = ffprobe_m4ag_tag(&dst);
    assert!(
        m4ag.contains("TAG:M4AG=m4againrs version=1 gain_steps=2 gain_step_db=1.5"),
        "expected M4AG tag missing: {m4ag}"
    );
    ffmpeg_decode_check(&dst);
}

#[test]
fn cli_rewrites_chunk_offsets_when_moov_grows_on_faststart_input() {
    // Faststart input has moov BEFORE mdat, so any moov growth from adding the
    // M4AG tag must shift mdat downward AND every stco/co64 chunk offset must
    // be rewritten by the same delta. Without this rewrite, the output decodes
    // garbage (or fails to decode) — both checked here.
    let tmp = TestDir::new("cli-faststart-chunk-offsets");
    let raw = tmp.join("raw.m4a");
    let faststart = tmp.join("faststart.m4a");
    let dst = tmp.join("faststart_out.m4a");
    ffmpeg_generate_tone(&raw);
    ffmpeg_remux_faststart(&raw, &faststart);

    let src_bytes = fs::read(&faststart).expect("failed to read faststart fixture");
    let src_moov = find_top_level_box(&src_bytes, b"moov").expect("missing moov in source");
    let src_mdat = find_top_level_box(&src_bytes, b"mdat").expect("missing mdat in source");
    assert!(
        src_moov.offset < src_mdat.offset,
        "fixture is not faststart"
    );
    let src_ranges = sample_byte_ranges(&src_bytes);

    let modified = m4againrs::aac_apply_gain_file(&faststart, &dst, 2)
        .expect("gain application should succeed on faststart input");
    assert!(modified > 0);

    let dst_bytes = fs::read(&dst).expect("failed to read gain-adjusted output");
    let dst_moov = find_top_level_box(&dst_bytes, b"moov").expect("missing moov in output");
    let dst_mdat = find_top_level_box(&dst_bytes, b"mdat").expect("missing mdat in output");
    let moov_growth = dst_moov.size - src_moov.size;
    assert!(
        moov_growth > 0,
        "moov did not grow; M4AG tag was apparently not added"
    );
    assert_eq!(
        dst_mdat.offset,
        src_mdat.offset + moov_growth,
        "mdat was not shifted by moov growth"
    );

    let dst_ranges = sample_byte_ranges(&dst_bytes);
    assert_eq!(src_ranges.len(), dst_ranges.len());
    for (idx, (&(src_off, src_size), &(dst_off, dst_size))) in
        src_ranges.iter().zip(&dst_ranges).enumerate()
    {
        assert_eq!(src_size, dst_size, "sample {idx} size changed");
        assert_eq!(
            dst_off,
            src_off + moov_growth,
            "sample {idx} offset not shifted by moov growth"
        );
    }
    ffmpeg_decode_check(&dst);
}

#[test]
fn cli_patches_short_window_stereo_frames_and_ffmpeg_decodes() {
    // Ported from Python: synthesize a stereo source whose two channels carry
    // independent transient bursts so the encoder is forced to emit a CPE
    // element with EIGHT_SHORT_SEQUENCE windows. Asserts both that the gain
    // patch hits every channel of every frame (modified == samples * 2) AND
    // that the output decodes cleanly.
    let tmp = TestDir::new("cli-short-window-stereo");
    let src = tmp.join("short_window.m4a");
    let dst = tmp.join("short_window_gain.m4a");

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
            "aevalsrc=\
'if(lt(mod(t,0.25),0.018),0.95*sin(2*PI*2800*t),0.04*sin(2*PI*440*t))|\
if(lt(mod(t,0.25),0.018),0.95*sin(2*PI*3600*t),0.04*sin(2*PI*660*t))'\
:s=32000:d=3",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
        ])
        .arg(&src)
        .status()
        .expect("failed to spawn ffmpeg to generate stereo transient source");
    assert!(status.success(), "ffmpeg failed to generate stereo transient");

    let src_bytes = fs::read(&src).expect("failed to read stereo transient fixture");
    let sample_count = sample_byte_ranges(&src_bytes).len();

    let modified = m4againrs::aac_apply_gain_file(&src, &dst, 5)
        .expect("gain application should succeed on stereo short-window input");
    assert_eq!(
        modified,
        sample_count * 2,
        "expected one gain patch per channel per sample (CPE element should yield 2 patches)"
    );

    ffmpeg_decode_check(&dst);
}

// ---------------------------------------------------------------------------
// Minimal MP4 box parser used by the moov/mdat-layout test above. Mirrors the
// equivalent helpers in tests/file_api.rs; kept private here so the CLI crate
// stays free-standing.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct BoxRange {
    offset: usize,
    size: usize,
    header_size: usize,
}

fn read_u32_be(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn read_u64_be(buf: &[u8], offset: usize) -> Option<u64> {
    let bytes = buf.get(offset..offset + 8)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

fn find_top_level_box(buf: &[u8], box_type: &[u8; 4]) -> Option<BoxRange> {
    find_box(buf, box_type, 0, buf.len())
}

fn find_child_box(buf: &[u8], parent: BoxRange, box_type: &[u8; 4]) -> Option<BoxRange> {
    find_box(
        buf,
        box_type,
        parent.offset + parent.header_size,
        parent.offset + parent.size,
    )
}

fn find_box(buf: &[u8], box_type: &[u8; 4], start: usize, end: usize) -> Option<BoxRange> {
    let end = end.min(buf.len());
    let mut off = start;

    while off.checked_add(8)? <= end {
        let small_size = read_u32_be(buf, off)? as usize;
        let mut header_size = 8usize;
        let size = match small_size {
            1 => {
                header_size = 16;
                usize::try_from(read_u64_be(buf, off + 8)?).ok()?
            }
            0 => end - off,
            _ => small_size,
        };
        let box_end = off.checked_add(size)?;
        if size < header_size || box_end > end {
            return None;
        }

        if buf[off + 4..off + 8] == box_type[..] {
            return Some(BoxRange {
                offset: off,
                size,
                header_size,
            });
        }

        off = box_end;
    }

    None
}

fn sample_byte_ranges(buf: &[u8]) -> Vec<(usize, usize)> {
    let moov = find_top_level_box(buf, b"moov").expect("missing moov box");
    let trak = find_child_box(buf, moov, b"trak").expect("missing trak box");
    let mdia = find_child_box(buf, trak, b"mdia").expect("missing mdia box");
    let minf = find_child_box(buf, mdia, b"minf").expect("missing minf box");
    let stbl = find_child_box(buf, minf, b"stbl").expect("missing stbl box");
    let stbl_start = stbl.offset + stbl.header_size;
    let stbl_end = stbl.offset + stbl.size;

    let stsz = find_box(buf, b"stsz", stbl_start, stbl_end).expect("missing stsz box");
    let stsc = find_box(buf, b"stsc", stbl_start, stbl_end).expect("missing stsc box");
    let stco = find_box(buf, b"stco", stbl_start, stbl_end);
    let co64 = if stco.is_none() {
        find_box(buf, b"co64", stbl_start, stbl_end)
    } else {
        None
    };

    let stsz_content = stsz.offset + stsz.header_size;
    let default_size = read_u32_be(buf, stsz_content + 4).expect("stsz missing default size");
    let sample_count =
        read_u32_be(buf, stsz_content + 8).expect("stsz missing sample count") as usize;
    let sample_sizes = if default_size != 0 {
        vec![default_size as usize; sample_count]
    } else {
        let sizes_start = stsz_content + 12;
        (0..sample_count)
            .map(|idx| {
                read_u32_be(buf, sizes_start + idx * 4).expect("stsz sample size missing") as usize
            })
            .collect()
    };

    let stsc_content = stsc.offset + stsc.header_size;
    let stsc_count = read_u32_be(buf, stsc_content + 4).expect("stsc missing entry count") as usize;
    let mut stsc_entries = Vec::with_capacity(stsc_count);
    for idx in 0..stsc_count {
        let off = stsc_content + 8 + idx * 12;
        stsc_entries.push((
            read_u32_be(buf, off).expect("stsc missing first chunk") as usize,
            read_u32_be(buf, off + 4).expect("stsc missing samples per chunk") as usize,
        ));
    }

    let chunk_offsets: Vec<usize> = if let Some(stco) = stco {
        let chunk_content = stco.offset + stco.header_size;
        let chunk_count =
            read_u32_be(buf, chunk_content + 4).expect("stco missing chunk count") as usize;
        (0..chunk_count)
            .map(|idx| {
                read_u32_be(buf, chunk_content + 8 + idx * 4).expect("stco chunk offset missing")
                    as usize
            })
            .collect()
    } else {
        let co64 = co64.expect("missing stco/co64 box");
        let chunk_content = co64.offset + co64.header_size;
        let chunk_count =
            read_u32_be(buf, chunk_content + 4).expect("co64 missing chunk count") as usize;
        (0..chunk_count)
            .map(|idx| {
                usize::try_from(
                    read_u64_be(buf, chunk_content + 8 + idx * 8)
                        .expect("co64 chunk offset missing"),
                )
                .expect("co64 offset does not fit in usize")
            })
            .collect()
    };

    let mut ranges = Vec::new();
    let mut sample_idx = 0usize;
    for (chunk_idx, chunk_off) in chunk_offsets.iter().copied().enumerate() {
        let chunk_num = chunk_idx + 1;
        let mut samples_per_chunk = stsc_entries[0].1;
        for &(first_chunk, count) in &stsc_entries {
            if first_chunk <= chunk_num {
                samples_per_chunk = count;
            } else {
                break;
            }
        }

        let mut off_in_chunk = 0usize;
        for _ in 0..samples_per_chunk {
            if sample_idx >= sample_count {
                break;
            }
            let size = sample_sizes[sample_idx];
            ranges.push((chunk_off + off_in_chunk, size));
            off_in_chunk += size;
            sample_idx += 1;
        }
    }

    ranges
}

