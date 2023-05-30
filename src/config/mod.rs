//! Configuration for a scrape run
//!

use clap::{Parser, value_parser};
use std::path::PathBuf;
use url::Url;
use crate::scrape::OutputFormat;

/// Scrape configuration
#[derive(Clone, Debug, Parser)]
#[command(
    name = "scrape-ct-log",
    about = "Fast, efficient scraping of Certificate Transparency logs",
    version
)]
pub struct Config {
	/// The base URL of the Certificate Transparency log to scrape
    #[arg(name = "log_url")]
    pub log_url: Url,

	/// The format of the output produced from the scrape
	#[arg(short, long, default_value_t = OutputFormat::default(), value_parser = |s: &str| OutputFormat::try_from(s))]
	pub format: OutputFormat,

	/// Write the scraped data to the specified file
	#[arg(short, long)]
	pub output: Option<PathBuf>,

	/// Include the submitted chain in the output
	#[arg(long, default_value = "false")]
	pub include_chains: bool,

	/// Include the raw precert data
	#[arg(long, default_value = "false")]
	pub include_precert_data: bool,

	/// The maximum number of entries to fetch from the log
	#[arg(short = 'n', long = "number-of-entries", value_parser = value_parser!(u64).range(1..=u64::MAX), default_value = "18446744073709551615")]
	pub count: u64,

	/// The first entry number to fetch from the log
	#[arg(short, long, value_parser = value_parser!(u64).range(0..=u64::MAX), default_value = "0")]
	pub start: u64,

	/// Increase the amount of informative and debugging output
	#[arg(short, long, action = clap::ArgAction::Count, default_value = "0")]
	pub verbose: u8,
}
