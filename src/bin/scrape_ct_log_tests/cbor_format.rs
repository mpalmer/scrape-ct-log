use assert_cmd::prelude::*;
use ciborium::Value;
use predicates::str::is_empty;
use std::collections::HashMap;
use std::time::Duration;
use x509_parser::{certificate::X509Certificate, prelude::FromDer as _};

use super::test_helpers::*;

fn parse_output(mut stdout: &[u8]) -> Value {
	ciborium::from_reader(&mut stdout).unwrap()
}

fn remap(m: &Value) -> HashMap<&str, &Value> {
	m.as_map()
		.expect("map to be a map")
		.into_iter()
		.map(|(k, v)| (k.as_text().expect(&format!("key {k:?} to be a string")), v))
		.collect()
}

fn intify(i: &ciborium::Value) -> u64 {
	i.as_integer()
		.expect("integer to be an integer")
		.try_into()
		.expect("integer to be reasonably sized")
}

#[test]
fn empty_log_produces_basic_output() {
	let log = faux_log(0..2);

	let log_url = { log.lock().unwrap().url() };

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-f", "cbor"])
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();
	res.assert().success();

	let output = parse_output(&stdout);
	let output_map = remap(&output);

	assert_eq!(
		log_url,
		output_map
			.get("log_url")
			.expect("output to have log_url")
			.as_text()
			.expect("log_url to be text")
	);
	assert!(output_map
		.get("scrape_begin_timestamp")
		.expect("stdout to have scrape_begin_timestamp")
		.is_integer());
	assert!(output_map
		.get("scrape_end_timestamp")
		.expect("stdout to have scrape_end_timestamp")
		.is_integer());
	assert!(
		output_map["scrape_begin_timestamp"].as_integer()
			<= output_map["scrape_end_timestamp"].as_integer()
	);

	let sth_map = remap(output_map.get("sth").expect("output to have sth"));
	assert_eq!(
		0u64,
		intify(sth_map.get("tree_size").expect("sth to have tree_size"))
	);
	assert_eq!(
		0u64,
		intify(sth_map.get("timestamp").expect("sth to have timestamp"))
	);
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
		.args(&["-f", "cbor"])
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	let output = parse_output(&stdout);
	let output_map = remap(&output);

	let entry = &output_map
		.get("entries")
		.expect("output to have entries")
		.as_array()
		.expect("entries to be an array")[0];
	assert!(entry.is_map());
	let entry_map = remap(entry);
	assert_eq!(0, intify(entry_map["entry_number"]));
	assert_eq!(1532471986235, intify(entry_map["timestamp"]));
	let der = entry_map["certificate"]
		.as_bytes()
		.expect("certificate to be a bytestring");
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!(
		"CN=Test7232018-1-1.msitvalidcert.com",
		x509_cert.subject().to_string()
	);

	assert!(entry_map.get("chain").is_none());
	assert!(entry_map.get("precert").is_none());
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
		.args(&["-f", "cbor"])
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();

	res.assert().success().stderr(is_empty());

	let output = parse_output(&stdout);
	let output_map = remap(&output);

	let entry = &output_map
		.get("entries")
		.expect("output to have entries")
		.as_array()
		.expect("entries to be an array")[0];
	assert!(entry.is_map());
	let entry_map = remap(entry);
	assert_eq!(0, intify(entry_map["entry_number"]));
	assert_eq!(1666198004098, intify(entry_map["timestamp"]));
	let der = entry_map["certificate"]
		.as_bytes()
		.expect("certificate to be a bytestring");
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!("CN=crt.sh", x509_cert.subject().to_string());

	assert!(entry_map.get("chain").is_none());
	assert!(entry_map.get("precert").is_none());
}
