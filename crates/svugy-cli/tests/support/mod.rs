//! Run the svugy binary on fixtures and scratch files.

use std::path::PathBuf;
use std::process::{Command, Output};

pub fn fixture(file_name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = PathBuf::from(manifest_dir)
        .join("tests/fixtures")
        .join(file_name);
    fixture_path.to_str().expect("utf-8 path").to_owned()
}

pub fn scratch_file(file_name: &str) -> String {
    let target_tmp = env!("CARGO_TARGET_TMPDIR");
    let scratch_path = PathBuf::from(target_tmp).join(file_name);
    scratch_path.to_str().expect("utf-8 path").to_owned()
}

pub fn run_svugy(arguments: &[&str]) -> Output {
    let svugy_binary = env!("CARGO_BIN_EXE_svugy");
    Command::new(svugy_binary)
        .args(arguments)
        .output()
        .expect("binary runs")
}

pub fn stderr_text(run: &Output) -> String {
    let stderr = run.stderr.clone();
    String::from_utf8(stderr).expect("utf-8 stderr")
}
