//! Fast, efficient scraping for Certificate Transparency logs
//!

pub mod file_writer;
pub mod processor;
pub mod runner;

pub(crate) mod fetcher;

mod error;
mod utils;

pub use error::Error;
pub use utils::fix_url;

// These deps are used in the binary, not the library
mod binary_deps {
	use clap as _;
	use flexi_logger as _;
}

// This isn't actually *used* anywhere, but we need to specify it as a dep
// so we can turn on the std feature
#[cfg(feature = "cbor")]
use ciborium_io as _;

#[cfg(test)]
mod dev_deps {
	// These dev deps are all used in testing the binary, not the library
	use assert_cmd as _;
	use base64 as _;
	use bytes as _;
	use http as _;
	use httptest as _;
	use hyper as _;
	use lazy_static as _;
	use predicates as _;
	use regex as _;
}
