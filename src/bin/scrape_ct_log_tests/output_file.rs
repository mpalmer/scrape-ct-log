use assert_cmd::prelude::*;
use predicates::str::is_empty;
use serde_json::Value as SerdeValue;
use std::time::Duration;

use super::test_helpers::*;

#[test]
fn writes_to_the_specified_file() {
	let log = faux_log(1..2);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(1, 1234567890, vec![0u8; 32], vec![0u8; 64]);
		mlog.add_entry(
			0,
			include_bytes!("precert_leaf_input"),
			include_bytes!("precert_extra_data"),
		);

		mlog.url()
	};

	let tmpdir = mktemp::Temp::new_dir().unwrap();
	let mut pathbuf = tmpdir.to_path_buf();
	pathbuf.push("log_output.json");
	let filepath = pathbuf.into_os_string().into_string().unwrap();

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-o", &filepath])
		.arg(log_url.clone())
		.unwrap();

	res.assert().success().stderr(is_empty());

	let reader = std::fs::File::open(filepath).unwrap();
	let output: SerdeValue = serde_json::from_reader(&reader).unwrap();

	assert!(output.is_object());
	assert_eq!(
		log_url,
		output
			.get("log_url")
			.expect("stdout to have log_url")
			.as_str()
			.expect("log_url to be a string")
	);

	let sth = output.get("sth").expect("stdout to have sth");
	assert_eq!(
		1,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);
	assert_eq!(
		1234567890,
		sth.get("timestamp")
			.expect("sth to have timestamp")
			.as_u64()
			.expect("timestamp to be a u64")
	);

	let entries = output
		.get("entries")
		.expect("stdout should have entries")
		.as_array()
		.expect("entries to be an array");
	assert_eq!(1, entries.len());
}
