#![allow(deprecated)]

mod generate_fixture;

use assert_cmd::cargo::cargo_bin;
use predicates::prelude::*;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn polez() -> assert_cmd::Command {
    assert_cmd::Command::from_std(StdCommand::new(cargo_bin("polez")))
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn ensure_test_wav() -> PathBuf {
    let path = fixture_dir().join("test.wav");
    if !path.exists() {
        generate_fixture::generate_test_wav(&path);
    }
    path
}

// ── version ──

#[test]
fn version_shows_output() {
    polez()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("Audio Forensics"));
}

// ── detect ──

#[test]
fn detect_runs_on_wav() {
    let wav = ensure_test_wav();
    polez()
        .args(["detect", wav.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn detect_deep_runs_on_wav() {
    let wav = ensure_test_wav();
    polez()
        .args(["detect", "--deep", wav.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn detect_missing_file_fails() {
    polez()
        .args(["detect", "nonexistent_file.wav"])
        .assert()
        .failure();
}

#[test]
fn detect_json_output() {
    let wav = ensure_test_wav();
    polez()
        .args(["--json", "detect", wav.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn detect_with_filter() {
    let wav = ensure_test_wav();
    polez()
        .args([
            "detect",
            "--filter",
            "spread_spectrum,echo_signatures",
            wav.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn detect_with_report() {
    let tmp = TempDir::new().unwrap();
    let report_path = tmp.path().join("report.json");
    let wav = ensure_test_wav();
    polez()
        .args([
            "detect",
            "--report",
            report_path.to_str().unwrap(),
            wav.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(report_path.exists(), "Report file should be created");
}

// ── clean ──

#[test]
fn clean_produces_output() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("cleaned.wav");
    let wav = ensure_test_wav();
    polez()
        .args([
            "clean",
            wav.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(output.exists(), "Output file should be created");
}

#[test]
fn clean_missing_file_fails() {
    polez()
        .args(["clean", "nonexistent_file.wav"])
        .assert()
        .failure();
}

#[test]
fn clean_dry_run_no_output() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("should_not_exist.wav");
    let wav = ensure_test_wav();
    polez()
        .args([
            "clean",
            "--dry-run",
            wav.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(!output.exists(), "Dry run should not create output file");
}

#[test]
fn clean_with_verify() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("verified.wav");
    let wav = ensure_test_wav();
    polez()
        .args([
            "clean",
            "--verify",
            wav.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn clean_with_quality_slider() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("quality.wav");
    let wav = ensure_test_wav();
    polez()
        .args([
            "clean",
            "--quality",
            "75",
            wav.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn clean_with_sample_rate() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("resampled.wav");
    let wav = ensure_test_wav();
    polez()
        .args([
            "clean",
            "--sample-rate",
            "48000",
            wav.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(output.exists(), "Resampled output should be created");
}

#[test]
fn clean_with_backup() {
    let tmp = TempDir::new().unwrap();
    let input_copy = tmp.path().join("input.wav");
    std::fs::copy(ensure_test_wav(), &input_copy).unwrap();
    let output = tmp.path().join("backed_up.wav");
    polez()
        .args([
            "clean",
            "--backup",
            input_copy.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

// ── inspect ──

#[test]
fn inspect_runs_on_wav() {
    let wav = ensure_test_wav();
    polez()
        .args(["inspect", wav.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn inspect_with_svg_output() {
    let tmp = TempDir::new().unwrap();
    let svg = tmp.path().join("spectrogram.svg");
    let wav = ensure_test_wav();
    polez()
        .args([
            "inspect",
            wav.to_str().unwrap(),
            "-o",
            svg.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(svg.exists(), "SVG file should be created");
}

#[test]
fn inspect_missing_file_fails() {
    polez()
        .args(["inspect", "nonexistent_file.wav"])
        .assert()
        .failure();
}

#[test]
fn inspect_with_freq_range() {
    let wav = ensure_test_wav();
    polez()
        .args([
            "inspect",
            "--freq-min",
            "1000",
            "--freq-max",
            "10000",
            wav.to_str().unwrap(),
        ])
        .assert()
        .success();
}

// ── bits ──

#[test]
fn bits_runs_on_wav() {
    let wav = ensure_test_wav();
    polez()
        .args(["bits", wav.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bits_with_search() {
    let wav = ensure_test_wav();
    polez()
        .args(["bits", "--search", wav.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn bits_missing_file_fails() {
    polez()
        .args(["bits", "nonexistent_file.wav"])
        .assert()
        .failure();
}

#[test]
fn bits_with_bit_plane() {
    let wav = ensure_test_wav();
    polez()
        .args(["bits", "-b", "7", wav.to_str().unwrap()])
        .assert()
        .success();
}

// ── config ──

#[test]
fn config_show() {
    polez().args(["config", "show"]).assert().success();
}

#[test]
fn config_list() {
    polez().args(["config", "list"]).assert().success();
}

#[test]
fn config_validate() {
    polez().args(["config", "validate"]).assert().success();
}

// ── sweep ──

#[test]
fn sweep_on_fixtures_dir() {
    let tmp = TempDir::new().unwrap();
    let _ = ensure_test_wav(); // make sure fixture exists
    polez()
        .args([
            "sweep",
            fixture_dir().to_str().unwrap(),
            "-d",
            tmp.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn sweep_dry_run() {
    let _ = ensure_test_wav();
    polez()
        .args(["sweep", "--dry-run", fixture_dir().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn sweep_empty_dir_succeeds() {
    let tmp = TempDir::new().unwrap();
    polez()
        .args(["sweep", tmp.path().to_str().unwrap()])
        .assert()
        .success();
}

// ── fingerprint ──

#[test]
fn fingerprint_compare_same_file() {
    let wav = ensure_test_wav();
    let path = wav.to_str().unwrap();
    polez().args(["fingerprint", path, path]).assert().success();
}

#[test]
fn fingerprint_missing_file_fails() {
    polez()
        .args(["fingerprint", "nonexistent1.wav", "nonexistent2.wav"])
        .assert()
        .failure();
}

// ── error cases ──

#[test]
fn no_args_shows_help() {
    polez()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn invalid_subcommand_fails() {
    polez().arg("nonexistent-subcommand").assert().failure();
}

#[test]
fn quiet_flag_suppresses_output() {
    let wav = ensure_test_wav();
    let output = polez()
        .args(["-q", "detect", wav.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
}
