use assert_cmd::prelude::*;
use base64::{engine::general_purpose::STANDARD_NO_PAD as b64, Engine as _};
use hex_literal::hex;
use predicates::str::is_empty;
use serde_json::Value as SerdeValue;
use std::time::Duration;
use x509_parser::{certificate::X509Certificate, prelude::FromDer as _};

use super::test_helpers::*;

#[test]
fn includes_precert_when_appropriate() {
	let log = faux_log(1..2);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(2, 1234567890, vec![0u8; 32], vec![0u8; 64]);
		mlog.add_entry(
			0,
			include_bytes!("precert_leaf_input"),
			include_bytes!("precert_extra_data"),
		);
		mlog.add_entry(
			1,
			include_bytes!("x509_leaf_input"),
			include_bytes!("x509_extra_data"),
		);

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.arg("--include-precert-data")
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();

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
		2,
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
	assert_eq!(2, entries.len());

	assert_eq!(
		hex!["d241053c 65bc85d5 1c270185 dafbe25c af36e849 7f9b50cb 501f3d18 c7950db2"],
		&b64.decode(
			entries[0]
				.get("precert")
				.expect("entry to have precert")
				.as_object()
				.expect("precert to be an object")
				.get("issuer_key_hash")
				.expect("precert to have issuer_key_hash")
				.as_str()
				.expect("issuer_key_hash to be a string")
		)
		.expect("issuer_key_hash to be valid base64")[..]
	);
	assert!(entries[0]
		.get("precert")
		.expect("entry to have precert")
		.as_object()
		.expect("precert to be an object")
		.get("tbs_certificate")
		.is_some());
	assert!(entries[0].get("chain").is_none());

	assert!(entries[1].get("precert").is_none());
	assert!(entries[1].get("chain").is_none());
}
