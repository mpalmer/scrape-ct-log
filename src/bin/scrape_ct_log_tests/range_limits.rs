use assert_cmd::prelude::*;
use base64::{engine::general_purpose::STANDARD_NO_PAD as b64, Engine as _};
use predicates::{prelude::PredicateBooleanExt, str::is_empty};
use serde_json::Value as SerdeValue;
use std::time::Duration;
use x509_parser::{certificate::X509Certificate, prelude::FromDer as _};

use super::test_helpers::*;

#[test]
fn number_of_entries_requires_a_positive_number() {
	for n in ["0", "-1", "3.14159625", "i"] {
		cmd()
			.args(&["-n", n])
			.assert()
			.failure()
			.stderr(is_empty().not());
	}
}

#[test]
fn limit_entries_fetches_only_some_entries() {
	let log = faux_log(1..2);

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

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-n", "10"])
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();
	res.assert().success().stderr(is_empty());

	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();
	assert!(output.is_object());

	let sth = output.get("sth").expect("stdout to have sth");
	assert_eq!(
		20,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);

	assert_eq!(10, output["entries"].as_array().unwrap().len());
	assert_eq!(
		0,
		output["entries"].as_array().unwrap()[0]
			.as_object()
			.unwrap()["entry_number"]
	);
	assert_eq!(
		9,
		output["entries"]
			.as_array()
			.unwrap()
			.iter()
			.last()
			.unwrap()
			.as_object()
			.unwrap()["entry_number"]
	);
}

#[test]
fn limit_entries_fetches_only_some_entries_in_chunks() {
	let log = faux_log(5..6);

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
		mlog.chunk_size = 2;

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-n", "10"])
		.arg(log_url.clone())
		.unwrap();

	let stdout = res.stdout.clone();
	res.assert().success().stderr(is_empty());

	let output: SerdeValue = serde_json::from_slice(&stdout).unwrap();
	assert!(output.is_object());

	let sth = output.get("sth").expect("stdout to have sth");
	assert_eq!(
		20,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);

	assert_eq!(10, output["entries"].as_array().unwrap().len());
	assert_eq!(
		0,
		output["entries"].as_array().unwrap()[0]
			.as_object()
			.unwrap()["entry_number"]
	);
	assert_eq!(
		9,
		output["entries"]
			.as_array()
			.unwrap()
			.iter()
			.last()
			.unwrap()
			.as_object()
			.unwrap()["entry_number"]
	);
}

#[test]
fn offset_fetches_from_the_right_place_to_the_end() {
	let log = faux_log(1..2);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(20, 1234567890, vec![0u8; 32], vec![0u8; 64]);
		for i in 0..10 {
			mlog.add_entry(
				i,
				include_bytes!("precert_leaf_input"),
				include_bytes!("precert_extra_data"),
			);
		}
		for i in 10..20 {
			mlog.add_entry(
				i,
				include_bytes!("x509_leaf_input"),
				include_bytes!("x509_extra_data"),
			);
		}

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-s", "10"])
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
		20,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);

	assert_eq!(10, output["entries"].as_array().unwrap().len());
	assert_eq!(
		10,
		output["entries"].as_array().unwrap()[0]
			.as_object()
			.unwrap()["entry_number"]
	);
	assert_eq!(
		19,
		output["entries"]
			.as_array()
			.unwrap()
			.iter()
			.last()
			.unwrap()
			.as_object()
			.unwrap()["entry_number"]
	);

	let der = b64
		.decode(
			output["entries"].as_array().unwrap()[0]
				.as_object()
				.unwrap()["certificate"]
				.as_str()
				.unwrap(),
		)
		.unwrap();
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!("CN=crt.sh", x509_cert.subject().to_string());
}

#[test]
fn limit_and_offset_work_together() {
	let log = faux_log(1..2);

	let log_url = {
		let mut mlog = log.lock().unwrap();

		mlog.sth(20, 1234567890, vec![0u8; 32], vec![0u8; 64]);
		for i in 0..10 {
			mlog.add_entry(
				i,
				include_bytes!("precert_leaf_input"),
				include_bytes!("precert_extra_data"),
			);
		}
		for i in 10..20 {
			mlog.add_entry(
				i,
				include_bytes!("x509_leaf_input"),
				include_bytes!("x509_extra_data"),
			);
		}

		mlog.url()
	};

	let res = cmd()
		.timeout(Duration::from_secs(1))
		.env("RUST_LOG", "warn")
		.args(&["-s", "5"])
		.args(&["-n", "10"])
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
		20,
		sth.get("tree_size")
			.expect("sth to have tree_size")
			.as_u64()
			.expect("tree_size to be a u64")
	);

	assert_eq!(10, output["entries"].as_array().unwrap().len());
	assert_eq!(
		5,
		output["entries"].as_array().unwrap()[0]
			.as_object()
			.unwrap()["entry_number"]
	);
	assert_eq!(
		14,
		output["entries"]
			.as_array()
			.unwrap()
			.iter()
			.last()
			.unwrap()
			.as_object()
			.unwrap()["entry_number"]
	);

	let der = b64
		.decode(
			output["entries"].as_array().unwrap()[4]
				.as_object()
				.unwrap()["certificate"]
				.as_str()
				.unwrap(),
		)
		.unwrap();
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!(
		"CN=Test7232018-1-1.msitvalidcert.com",
		x509_cert.subject().to_string()
	);

	let der = b64
		.decode(
			output["entries"].as_array().unwrap()[5]
				.as_object()
				.unwrap()["certificate"]
				.as_str()
				.unwrap(),
		)
		.unwrap();
	let (_, x509_cert) = X509Certificate::from_der(&der).unwrap();
	assert_eq!("CN=crt.sh", x509_cert.subject().to_string());
}
