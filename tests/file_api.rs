use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX_EPOCH")
            .as_nanos();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join("rust-tests")
            .join(format!("{name}-{}-{now}", std::process::id()));
        fs::create_dir_all(&path).expect("failed to create test directory under ./tmp");
        Self { path }
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn file_api_rejects_zero_gain_before_touching_paths() {
    let tmp = TestDir::new("file-api-zero-gain");
    let src = tmp.join("missing.m4a");
    let dst = tmp.join("out.m4a");

    let err = m4againrs::aac_apply_gain_file(&src, &dst, 0).unwrap_err();

    assert!(matches!(err, m4againrs::Error::ZeroGainSteps));
    assert!(!dst.exists());
}

#[test]
fn file_api_rejects_same_source_and_destination() {
    let tmp = TestDir::new("file-api-same-path");
    let src = tmp.join("same.m4a");

    let err = m4againrs::aac_apply_gain_file(&src, &src, 2).unwrap_err();

    assert!(matches!(err, m4againrs::Error::SameSourceDestination));
}

#[test]
fn file_api_rejects_non_mp4_without_creating_destination() {
    let tmp = TestDir::new("file-api-non-mp4");
    let src = tmp.join("input.bin");
    let dst = tmp.join("out.m4a");
    fs::write(&src, b"not an mp4").expect("failed to write non-MP4 fixture");

    let err = m4againrs::aac_apply_gain_file(&src, &dst, 2).unwrap_err();

    assert!(matches!(err, m4againrs::Error::NotMp4));
    assert!(!dst.exists());
}

#[test]
fn file_api_applies_positive_and_negative_gain_without_touching_source() {
    let tmp = TestDir::new("file-api-mutates");

    for steps in [2, -2] {
        let src = tmp.join(format!("in-{steps}.m4a"));
        let dst = tmp.join(format!("out-{steps}.m4a"));
        copy_fixture(&testdata_path("test.m4a"), &src);
        let src_bytes = fs::read(&src).expect("failed to read source fixture");
        let src_ranges = sample_byte_ranges(&src_bytes);

        let modified = m4againrs::aac_apply_gain_file(&src, &dst, steps)
            .expect("gain application should succeed");

        assert!(modified > 0);
        assert_eq!(fs::read(&src).expect("failed to reread source"), src_bytes);

        let dst_bytes = fs::read(&dst).expect("failed to read output fixture");
        let dst_ranges = sample_byte_ranges(&dst_bytes);
        assert_same_sample_sizes(&src_ranges, &dst_ranges);
        assert!(
            sample_payloads_differ(&src_bytes, &dst_bytes, &src_ranges, &dst_ranges),
            "no AAC sample bytes changed for gain_steps={steps}"
        );
    }
}

#[test]
fn file_api_preserves_ftyp_and_sample_sizes() {
    let tmp = TestDir::new("file-api-layout");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    copy_fixture(&testdata_path("test.m4a"), &src);

    let src_bytes = fs::read(&src).expect("failed to read source fixture");
    let src_ftyp = top_level_box_bytes(&src_bytes, b"ftyp").to_vec();
    let src_ranges = sample_byte_ranges(&src_bytes);

    m4againrs::aac_apply_gain_file(&src, &dst, 3).expect("gain application should succeed");

    let dst_bytes = fs::read(&dst).expect("failed to read output fixture");
    assert_eq!(
        top_level_box_bytes(&dst_bytes, b"ftyp"),
        src_ftyp.as_slice()
    );
    assert_same_sample_sizes(&src_ranges, &sample_byte_ranges(&dst_bytes));
}

#[test]
fn file_api_preserves_existing_tags_and_adds_gain_tag() {
    let tmp = TestDir::new("file-api-tags");
    let src = tmp.join("tagged.m4a");
    let dst = tmp.join("tagged-out.m4a");
    copy_fixture(&testdata_path("tagged_tone.m4a"), &src);

    let src_bytes = fs::read(&src).expect("failed to read tagged fixture");
    let mut expected_tags = parse_itunes_tags(&src_bytes);
    assert!(expected_tags.contains_key(b"\xa9nam".as_slice()));
    expected_tags.insert(
        b"M4AG".to_vec(),
        b"m4againrs version=1 gain_steps=4 gain_step_db=1.5".to_vec(),
    );

    let modified =
        m4againrs::aac_apply_gain_file(&src, &dst, 4).expect("gain application should succeed");

    assert!(modified > 0);
    assert_eq!(fs::read(&src).expect("failed to reread source"), src_bytes);
    assert_eq!(
        parse_itunes_tags(&fs::read(&dst).expect("failed to read output")),
        expected_tags
    );
}

#[test]
fn writer_api_accepts_forward_only_output() {
    let fixture = testdata_path("test.m4a");
    let src_bytes = fs::read(&fixture).expect("failed to read fixture");
    let src_ranges = sample_byte_ranges(&src_bytes);
    let mut src = fs::File::open(&fixture).expect("failed to open fixture");
    let mut dst = Vec::new();

    let modified = m4againrs::aac_apply_gain_to_writer(&mut src, &mut dst, 2)
        .expect("writer API should accept non-seekable output");

    assert!(modified > 0);
    let dst_ranges = sample_byte_ranges(&dst);
    assert_same_sample_sizes(&src_ranges, &dst_ranges);
    assert!(
        sample_payloads_differ(&src_bytes, &dst, &src_ranges, &dst_ranges),
        "no AAC sample bytes changed"
    );
    assert_eq!(
        parse_itunes_tags(&dst).get(b"M4AG".as_slice()),
        Some(&b"m4againrs version=1 gain_steps=2 gain_step_db=1.5".to_vec())
    );
}

#[test]
fn streaming_api_rejects_zero_gain_steps() {
    let mut src = Cursor::new(Vec::<u8>::new());
    let mut dst = Vec::new();

    let err = m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, 0).unwrap_err();

    assert!(matches!(err, m4againrs::Error::ZeroGainSteps));
    assert!(dst.is_empty());
}

#[test]
fn streaming_api_rejects_non_faststart_input() {
    // The bundled test.m4a is non-faststart (mdat before moov). Confirm that
    // the streaming API rejects it without writing any audio bytes — only the
    // ftyp header (and any pre-mdat free box) may have been streamed before
    // mdat is encountered.
    let bytes = fs::read(testdata_path("test.m4a")).expect("failed to read fixture");
    let mut src = Cursor::new(&bytes);
    let mut dst = Vec::new();

    let err = m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, 2).unwrap_err();

    assert!(matches!(err, m4againrs::Error::NonFaststartInput));
    // Pre-moov boxes (ftyp, free) may be in dst; mdat must not be.
    assert!(!dst.windows(4).any(|w| w == b"mdat"));
}

#[test]
fn streaming_api_patches_faststart_fixture() {
    let fixture = testdata_path("test_faststart.m4a");
    let bytes = fs::read(&fixture).expect("failed to read faststart fixture");
    let src_ranges = sample_byte_ranges(&bytes);
    let mut src = Cursor::new(&bytes);
    let mut dst = Vec::new();

    let modified = m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, 3)
        .expect("streaming API should accept faststart input");

    assert!(modified > 0);
    let dst_ranges = sample_byte_ranges(&dst);
    assert_same_sample_sizes(&src_ranges, &dst_ranges);
    assert!(
        sample_payloads_differ(&bytes, &dst, &src_ranges, &dst_ranges),
        "no AAC sample bytes changed"
    );
    assert_eq!(
        parse_itunes_tags(&dst).get(b"M4AG".as_slice()),
        Some(&b"m4againrs version=1 gain_steps=3 gain_step_db=1.5".to_vec())
    );
}

#[test]
fn streaming_api_matches_writer_api_for_faststart_input() {
    // The load-bearing correctness check: for faststart input, both APIs share
    // the same moov rewrite, sample table, and patch math, so the outputs MUST
    // be byte-identical.
    let bytes = fs::read(testdata_path("test_faststart.m4a"))
        .expect("failed to read faststart fixture");

    let mut writer_out = Vec::new();
    let writer_modified =
        m4againrs::aac_apply_gain_to_writer(&mut Cursor::new(&bytes), &mut writer_out, 3)
            .expect("writer API should accept faststart input");

    let mut streaming_out = Vec::new();
    let streaming_modified =
        m4againrs::aac_apply_gain_streaming(&mut Cursor::new(&bytes), &mut streaming_out, 3)
            .expect("streaming API should accept faststart input");

    assert_eq!(writer_modified, streaming_modified);
    assert_eq!(writer_out, streaming_out);
}

#[test]
fn file_api_applies_gain_to_he_aacv2_fixture() {
    let tmp = TestDir::new("file-api-he-aacv2");

    for steps in [2, -2] {
        let src = tmp.join(format!("in-{steps}.m4a"));
        let dst = tmp.join(format!("out-{steps}.m4a"));
        copy_fixture(&testdata_path("he_aacv2.m4a"), &src);
        let src_bytes = fs::read(&src).expect("failed to read source fixture");
        let src_ranges = sample_byte_ranges(&src_bytes);

        let modified = m4againrs::aac_apply_gain_file(&src, &dst, steps)
            .expect("gain application should succeed for HE-AACv2");

        assert!(modified > 0);
        assert_eq!(fs::read(&src).expect("failed to reread source"), src_bytes);

        let dst_bytes = fs::read(&dst).expect("failed to read output fixture");
        let dst_ranges = sample_byte_ranges(&dst_bytes);
        assert_same_sample_sizes(&src_ranges, &dst_ranges);
        assert!(
            sample_payloads_differ(&src_bytes, &dst_bytes, &src_ranges, &dst_ranges),
            "no AAC sample bytes changed for HE-AACv2 gain_steps={steps}"
        );
    }
}

#[test]
fn streaming_api_patches_he_aacv2_faststart_fixture() {
    let bytes = fs::read(testdata_path("he_aacv2_faststart.m4a"))
        .expect("failed to read HE-AACv2 faststart fixture");
    let src_ranges = sample_byte_ranges(&bytes);
    let mut src = Cursor::new(&bytes);
    let mut dst = Vec::new();

    let modified = m4againrs::aac_apply_gain_streaming(&mut src, &mut dst, 3)
        .expect("streaming API should accept HE-AACv2 faststart input");

    assert!(modified > 0);
    let dst_ranges = sample_byte_ranges(&dst);
    assert_same_sample_sizes(&src_ranges, &dst_ranges);
    assert!(
        sample_payloads_differ(&bytes, &dst, &src_ranges, &dst_ranges),
        "no AAC sample bytes changed for HE-AACv2"
    );
    assert_eq!(
        parse_itunes_tags(&dst).get(b"M4AG".as_slice()),
        Some(&b"m4againrs version=1 gain_steps=3 gain_step_db=1.5".to_vec())
    );
}

#[test]
fn file_api_applies_gain_to_aac_lc_51_fixture() {
    // 5.1 layout exercises the ID_SCE (front center) and ID_LFE branches in
    // parse_raw_data_block, which are not hit by any of the stereo fixtures.
    let tmp = TestDir::new("file-api-aac-lc-51");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    copy_fixture(&testdata_path("aac_lc_51.m4a"), &src);
    let src_bytes = fs::read(&src).expect("failed to read 5.1 fixture");
    let src_ranges = sample_byte_ranges(&src_bytes);

    let modified = m4againrs::aac_apply_gain_file(&src, &dst, 2)
        .expect("gain application should succeed for AAC LC 5.1");

    assert!(modified > 0);
    let dst_bytes = fs::read(&dst).expect("failed to read 5.1 output");
    let dst_ranges = sample_byte_ranges(&dst_bytes);
    assert_same_sample_sizes(&src_ranges, &dst_ranges);
    assert!(
        sample_payloads_differ(&src_bytes, &dst_bytes, &src_ranges, &dst_ranges),
        "no AAC sample bytes changed for 5.1 fixture"
    );
}

#[test]
fn file_api_applies_gain_to_short_window_fixture() {
    // The transient fixture forces the encoder into EIGHT_SHORT_SEQUENCE for a
    // significant fraction of frames (verified out-of-band: ~24% short windows).
    // Without it, the !long_win branches in parse_ics_info / parse_section_data
    // / spectral parsing are dead in our test suite.
    let tmp = TestDir::new("file-api-aac-lc-transient");
    let src = tmp.join("in.m4a");
    let dst = tmp.join("out.m4a");
    copy_fixture(&testdata_path("aac_lc_transient.m4a"), &src);
    let src_bytes = fs::read(&src).expect("failed to read transient fixture");
    let src_ranges = sample_byte_ranges(&src_bytes);

    let modified = m4againrs::aac_apply_gain_file(&src, &dst, 2)
        .expect("gain application should succeed for short-window content");

    assert!(modified > 0);
    let dst_bytes = fs::read(&dst).expect("failed to read transient output");
    let dst_ranges = sample_byte_ranges(&dst_bytes);
    assert_same_sample_sizes(&src_ranges, &dst_ranges);
    assert!(
        sample_payloads_differ(&src_bytes, &dst_bytes, &src_ranges, &dst_ranges),
        "no AAC sample bytes changed for transient fixture"
    );
}

#[test]
fn streaming_api_matches_writer_api_for_he_aacv2_faststart_input() {
    let bytes = fs::read(testdata_path("he_aacv2_faststart.m4a"))
        .expect("failed to read HE-AACv2 faststart fixture");

    let mut writer_out = Vec::new();
    let writer_modified =
        m4againrs::aac_apply_gain_to_writer(&mut Cursor::new(&bytes), &mut writer_out, 3)
            .expect("writer API should accept HE-AACv2 faststart input");

    let mut streaming_out = Vec::new();
    let streaming_modified =
        m4againrs::aac_apply_gain_streaming(&mut Cursor::new(&bytes), &mut streaming_out, 3)
            .expect("streaming API should accept HE-AACv2 faststart input");

    assert_eq!(writer_modified, streaming_modified);
    assert_eq!(writer_out, streaming_out);
}

fn copy_fixture(src: &Path, dst: &Path) {
    fs::copy(src, dst).expect("failed to copy fixture");
}

fn testdata_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name)
}

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

fn top_level_box_bytes<'a>(buf: &'a [u8], box_type: &[u8; 4]) -> &'a [u8] {
    let box_range = find_top_level_box(buf, box_type).expect("missing top-level box");
    &buf[box_range.offset..box_range.offset + box_range.size]
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

fn assert_same_sample_sizes(left: &[(usize, usize)], right: &[(usize, usize)]) {
    assert_eq!(left.len(), right.len());
    for (idx, (&(_, left_size), &(_, right_size))) in left.iter().zip(right).enumerate() {
        assert_eq!(left_size, right_size, "sample {idx} size changed");
    }
}

fn sample_payloads_differ(
    before: &[u8],
    after: &[u8],
    before_ranges: &[(usize, usize)],
    after_ranges: &[(usize, usize)],
) -> bool {
    before_ranges
        .iter()
        .zip(after_ranges)
        .any(|(&(before_off, size), &(after_off, _))| {
            before[before_off..before_off + size] != after[after_off..after_off + size]
        })
}

fn parse_itunes_tags(buf: &[u8]) -> HashMap<Vec<u8>, Vec<u8>> {
    let Some(moov) = find_top_level_box(buf, b"moov") else {
        return HashMap::new();
    };
    let Some(udta) = find_child_box(buf, moov, b"udta") else {
        return HashMap::new();
    };
    let Some(meta) = find_child_box(buf, udta, b"meta") else {
        return HashMap::new();
    };
    let Some(ilst) = find_box(
        buf,
        b"ilst",
        meta.offset + meta.header_size + 4,
        meta.offset + meta.size,
    ) else {
        return HashMap::new();
    };

    let mut tags = HashMap::new();
    let mut off = ilst.offset + ilst.header_size;
    let ilst_end = ilst.offset + ilst.size;

    while off + 8 <= ilst_end {
        let Some(item_size) = read_u32_be(buf, off).map(|value| value as usize) else {
            break;
        };
        let item_end = match off.checked_add(item_size) {
            Some(end) if item_size >= 8 && end <= ilst_end => end,
            _ => break,
        };
        let item_name = buf[off + 4..off + 8].to_vec();

        if let Some(data_box) = find_box(buf, b"data", off + 8, item_end) {
            let data_start = data_box.offset + data_box.header_size + 8;
            let data_end = data_box.offset + data_box.size;
            if data_start <= data_end && data_end <= buf.len() {
                tags.insert(item_name, buf[data_start..data_end].to_vec());
            }
        }

        off = item_end;
    }

    tags
}
