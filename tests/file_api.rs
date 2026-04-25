use std::fs;
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

    fn join(&self, path: &str) -> PathBuf {
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
