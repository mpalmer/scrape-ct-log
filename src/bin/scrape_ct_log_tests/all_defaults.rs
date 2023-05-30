use assert_cmd::prelude::*;
use base64::{engine::general_purpose::STANDARD_NO_PAD as b64, Engine as _};
use predicates::str::is_empty;
use serde_json::Value as SerdeValue;
use std::time::Duration;
use x509_parser::{certificate::X509Certificate, prelude::FromDer as _};

use super::test_helpers::*;

#[test]
fn empty_log_produces_basic_output() {
	let log = faux_log(0..2);

	let log_url = { log.lock().unwrap().url() };

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
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
	assert!(output
		.get("scrape_begin_timestamp")
		.expect("stdout to have scrape_begin_timestamp")
		.is_u64());
	assert!(output
		.get("scrape_end_timestamp")
		.expect("stdout to have scrape_end_timestamp")
		.is_u64());
	assert!(output["scrape_begin_timestamp"].as_u64() <= output["scrape_end_timestamp"].as_u64());

	let sth = output.get("sth").expect("stdout to have sth");
	assert_eq!(
		0,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);
	assert_eq!(
		0,
		sth.get("timestamp")
			.expect("sth to have timestamp")
			.as_u64()
			.expect("timestamp to be a u64")
	);
}

#[test]
fn log_with_one_entry_produces_one_output_entry() {
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

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
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

#[test]
fn precert_log_entry_is_properly_decoded() {
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

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	dbg!(String::from_utf8(stdout.clone()).unwrap());
	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();

	let entry = &output["entries"][0];
	assert!(entry.is_object());
	assert_eq!(0, entry["entry_number"].as_u64().unwrap());
	assert_eq!(1532471986235, entry["timestamp"].as_u64().unwrap());
	let der = b64.decode(entry["certificate"].as_str().unwrap()).unwrap();
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!(
		"CN=Test7232018-1-1.msitvalidcert.com",
		x509_cert.subject().to_string()
	);

	assert!(entry.get("chain").is_none());
	assert!(entry.get("precert").is_none());
}

#[test]
fn x509_log_entry_is_properly_decoded() {
	let log = faux_log(1..2);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(1, 9876543210, vec![0u8; 32], vec![0u8; 64]);
		mlog.add_entry(
			0,
			include_bytes!("x509_leaf_input"),
			include_bytes!("x509_extra_data"),
		);

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	dbg!(String::from_utf8(stdout.clone()).unwrap());
	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();

	let entry = &output["entries"][0];
	assert!(entry.is_object());
	assert_eq!(0, entry["entry_number"].as_u64().unwrap());
	assert_eq!(1666198004098, entry["timestamp"].as_u64().unwrap());
	let der = b64.decode(entry["certificate"].as_str().unwrap()).unwrap();
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!("CN=crt.sh", x509_cert.subject().to_string());

	assert!(entry.get("chain").is_none());
	assert!(entry.get("precert").is_none());
}

#[test]
fn scrapes_multiple_chunks() {
	let log = faux_log(4..5);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(20, 1234567890, vec![0u8; 32], vec![0u8; 64]);
		for i in 0..20 {
			mlog.add_entry(
				i,
				include_bytes!("precert_leaf_input"),
				include_bytes!("precert_extra_data"),
			);
		}
		mlog.chunk_size = 5;

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	dbg!(String::from_utf8(stdout.clone()).unwrap());
	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();

	assert_eq!(20, output["entries"].as_array().unwrap().len());
}
