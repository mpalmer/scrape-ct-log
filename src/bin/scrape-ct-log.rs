//! A command-line tool to scrape Certificate Transparency logs

use clap::{value_parser, Parser};
use scrape_ct_log::{
	file_writer::{self, FileWriter, OutputFormat},
	fix_url, runner,
};
use std::path::PathBuf;
use std::process::exit;
use url::Url;

#[derive(Clone, Debug, Parser)]
#[command(
	name = "scrape-ct-log",
	about = "Fast, efficient scraping of Certificate Transparency logs",
	version
)]
struct Config {
	/// The base URL of the Certificate Transparency log to scrape
	#[arg(name = "log_url")]
	log_url: Url,

	/// The format of the output produced from the scrape
	#[arg(short, long, default_value_t = OutputFormat::default(), value_parser = |s: &str| OutputFormat::try_from(s))]
	format: OutputFormat,

	/// Write the scraped data to the specified file
	#[arg(short, long)]
	output: Option<PathBuf>,

	/// Include the submitted chain in the output
	#[arg(long, default_value = "false")]
	include_chains: bool,

	/// Include the raw precert data
	#[arg(long, default_value = "false")]
	include_precert_data: bool,

	/// The maximum number of entries to fetch from the log
	#[arg(short = 'n', long = "number-of-entries", value_parser = value_parser!(u64).range(1..=u64::MAX), default_value = "18446744073709551615")]
	count: u64,

	/// The first entry number to fetch from the log
	#[arg(short, long, value_parser = value_parser!(u64).range(0..=u64::MAX), default_value = "0")]
	start: u64,

	/// Increase the amount of informative and debugging output
	#[arg(short, long, action = clap::ArgAction::Count, default_value = "0")]
	verbose: u8,
}

const LOG_VERBOSITY_CONFIG: &[&str] = &["warn", "info", "debug", "trace, rustls=debug"];

fn main() {
	let cfg = Config::parse();

	#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
	// If this craps out, we have many problems
	flexi_logger::Logger::try_with_env_or_str(LOG_VERBOSITY_CONFIG[cfg.verbose as usize])
		.unwrap()
		.adaptive_format_for_stderr(flexi_logger::AdaptiveFormat::WithThread)
		.start()
		.unwrap();

	let writer: Box<dyn std::io::Write + Send + Sync> = if let Some(output_file) = &cfg.output {
		match std::fs::File::create(output_file) {
			Ok(writer) => Box::new(writer),
			Err(e) => {
				log::error!(
					"Could not open output file {}: {}",
					output_file.display(),
					e
				);
				exit(1);
			}
		}
	} else {
		Box::new(std::io::stdout())
	};

	let args = file_writer::Args::new(writer, fix_url(cfg.log_url.clone()))
		.include_precert_data(cfg.include_precert_data)
		.include_chains(cfg.include_chains)
		.format(cfg.format);
	let run_config = runner::Config::new(fix_url(cfg.log_url))
		.user_agent("scrape-ct-log/0.0.0")
		.limit(cfg.count)
		.offset(cfg.start);

	if let Err(e) = runner::run::<FileWriter<'_, _>>(&run_config, args) {
		log::error!("Scrape failed: {e}");
		exit(1);
	}
}

#[cfg(test)]
mod scrape_ct_log_tests;

// library-only deps
use base64 as _;
use ct_structs as _;
use gen_server as _;
use num as _;
use rand as _;
use serde_json as _;
use thiserror as _;
use ureq as _;

#[cfg(feature = "cbor")]
use ciborium_io as _;
#[cfg(feature = "cbor")]
use ciborium_ll as _;
