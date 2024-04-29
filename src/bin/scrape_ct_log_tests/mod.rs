//! End-to-end integration tests for the scrape-ct-log CLI.
//!

mod faux_log;
mod test_helpers;

mod all_defaults;
mod basic;
mod include_chains;
mod include_precert_data;
mod output_file;
mod range_limits;

#[cfg(feature = "cbor")]
mod cbor_format;

// CBOR-only test dependency
#[cfg(not(feature = "cbor"))]
use ciborium as _;
