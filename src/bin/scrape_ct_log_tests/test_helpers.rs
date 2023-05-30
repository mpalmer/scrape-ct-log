use assert_cmd::Command;
use lazy_static::lazy_static;
use std::ffi::OsString;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use super::faux_log::FauxLog;

use itertools::Itertools;

lazy_static! {
	static ref COMMAND_PATH: OsString = {
		// This is bonkers
		let features = <std::vec::IntoIter<&str> as Itertools>::join(&mut vec![
			#[cfg(feature = "cbor")]
			"cbor",
		].into_iter(), ",");

		assert!(std::process::Command::new("cargo").arg("build").arg("--bin").arg("scrape-ct-log").arg("--no-default-features").arg("--features").arg(features).status().expect("build failed").success(), "binary build failed");
		assert_cmd::cargo::cargo_bin(env!("CARGO_PKG_NAME")).into_os_string()
	};
}

pub(crate) fn cmd() -> Command {
	Command::new(COMMAND_PATH.to_str().unwrap())
}

pub(crate) fn faux_log(expected_entries_requests: Range<usize>) -> Arc<Mutex<FauxLog<'static>>> {
	FauxLog::new(expected_entries_requests)
}
