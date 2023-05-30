use super::test_helpers::*;
use predicates::str::contains;

#[test]
fn errors_on_zero_args() {
	cmd()
		.assert()
		.code(2)
		.stderr(contains("<log_url>"))
		.stderr(contains("--help"));
}

#[test]
fn errors_on_unknown_option() {
	cmd()
		.arg("--this-option-is-unknown")
		.assert()
		.code(2)
		.stderr(contains("--this-option-is-unknown"))
		.stderr(contains("--help"));
}

#[test]
fn provides_help() {
	for opt in vec!["-h", "--help"] {
		cmd()
			.arg(opt)
			.assert()
			.success()
			.stdout(contains("scrape-ct-log"))
			.stdout(contains("--help"))
			.stdout(contains("Print help"))
			.stdout(contains("Print version"));
	}
}

#[test]
fn provides_a_version() {
	for opt in vec!["-V", "--version"] {
		cmd()
			.arg(opt)
			.assert()
			.success()
			.stdout(contains("scrape-ct-log"))
			.stdout(contains("0.0.0-git"));
	}
}
