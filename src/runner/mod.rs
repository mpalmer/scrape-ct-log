//! Where the magic happens
//!
//! All the functionality for the actual scrape work happens in here.
//!

use ct_structs::v1::response::GetSth as GetSthResponse;
use gen_server::GenServer;
use num::integer::div_floor;
use std::any::type_name;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::sync::mpsc;
use std::thread::available_parallelism;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

use crate::{
	error::Error,
	fetcher::{FetchStatus, Fetcher},
	fix_url, processor,
};

const MIN_BATCH_SIZE: u64 = 100;
const MAX_BATCH_SIZE: u64 = 10_000;
const SUCCESS_STEP: usize = 5;

#[derive(Clone, Debug)]
pub(crate) struct RunCtl {
	tx: mpsc::Sender<FetchStatus>,
}

impl RunCtl {
	fn new() -> (mpsc::Receiver<FetchStatus>, Self) {
		let (tx, rx) = mpsc::channel();

		(rx, RunCtl { tx })
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn success(&self) -> Result<(), Error> {
		log::debug!("Telling the runner we succeeded");
		self.tx
			.send(FetchStatus::Success)
			.map_err(|e| Error::system("failed to send status message to runner", e))
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn failure(&self) -> Result<(), Error> {
		log::debug!("Telling the runner we failed");
		self.tx
			.send(FetchStatus::Failure)
			.map_err(|e| Error::system("failed to send status message to runner", e))
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn complete(&self, n: usize) -> Result<(), Error> {
		log::debug!("Telling the runner we've finished this chunk");
		self.tx
			.send(FetchStatus::Complete(n))
			.map_err(|e| Error::system("failed to send status message to runner", e))
	}
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Config {
	log_url: Url,
	user_agent: String,
	limit: u64,
	offset: u64,
	initial_fetchers: usize,
	max_fetchers: Option<usize>,
}

impl Config {
	#[must_use]
	pub fn new(log_url: Url) -> Self {
		Config {
			log_url,
			user_agent: String::new(),
			limit: 0,
			offset: 0,
			initial_fetchers: 1,
			max_fetchers: None,
		}
	}

	#[must_use]
	pub fn user_agent<S: std::fmt::Display>(mut self, user_agent: S) -> Self {
		self.user_agent = user_agent.to_string();
		self
	}

	#[must_use]
	pub fn limit(mut self, limit: u64) -> Self {
		self.limit = limit;
		self
	}

	#[must_use]
	pub fn offset(mut self, offset: u64) -> Self {
		self.offset = offset;
		self
	}

	#[must_use]
	pub fn initial_fetchers(mut self, fetchers: usize) -> Self {
		self.initial_fetchers = fetchers;
		self
	}

	#[must_use]
	pub fn max_fetchers(mut self, max_fetchers: usize) -> Self {
		self.max_fetchers = Some(max_fetchers);
		self
	}
}

#[derive(Debug, Default)]
#[non_exhaustive]
pub struct RunStats {
	pub fetched_count: u64,
	pub sth_retrieved_at: u64,
	pub sth_timestamp: u64,
	pub sth_tree_size: u64,
}

/// Run a scrape according to the specified configuration, feeding the entries
/// received to a `GenServer` of the given type.
///
#[allow(clippy::result_large_err)] // Oh shoosh
#[allow(clippy::too_many_lines)] // TODO: refactor
pub fn run<O>(cfg: &Config, args: O::Args) -> Result<RunStats, Error>
where
	O: GenServer<Request = processor::Request, StopReason = ()> + Send + Sync + 'static,
{
	let mut stats = RunStats::default();

	log::debug!("Running a scrape with configuration: {cfg:?}");

	let log_url = fix_url(cfg.log_url.clone());

	let sth_url = log_url
		.join("ct/v1/get-sth")
		.map_err(|e| Error::URLError("STH".to_string(), e))?;
	log::debug!("Using STH URL {sth_url:?}");
	let sth_response = ureq::get(sth_url.as_ref())
		.call()
		.map_err(Error::RequestError)?;

	let sth: GetSthResponse = serde_json::from_reader(sth_response.into_reader())
		.map_err(|e| Error::json_parse("get-sth response", e))?;

	log::info!("Fetched STH; tree_size={}", sth.tree_size);
	let tree_size = sth.tree_size;

	#[allow(clippy::expect_used)] // I'll take the risk
	{
		stats.sth_tree_size = sth.tree_size;
		stats.sth_timestamp = sth.timestamp;
		stats.sth_retrieved_at = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_err(|e| Error::system("we went back in time somehow", e))?
			.as_millis()
			.try_into()
			.expect("wow this code has excellent shelf life");
	}

	let o = gen_server::start::<O>(args)
		.map_err(|e| Error::system(format!("failed to start {}", type_name::<O>()), e))?;
	o.cast(processor::Request::Metadata(sth));

	stats.fetched_count = if cfg.offset >= tree_size {
		log::warn!("Not fetching any entries because the log's tree_size {tree_size} is less than the requested start position {}", cfg.offset);
		0
	} else {
		let max_fetchers = if let Some(max) = cfg.max_fetchers {
			max
		} else {
			available_parallelism().map_or_else(
				|e| {
					log::warn!("Unable to determine available parallelism: {e}");
					1
				},
				std::num::NonZeroUsize::get,
			)
		};
		log::info!("Using up to {max_fetchers} parallel fetchers");

		let last_entry = min(tree_size, cfg.offset.saturating_add(cfg.limit))
			.checked_sub(1)
			.ok_or_else(|| Error::arithmetic("adjusting last_entry"))?;

		let mut fetchers: Vec<Fetcher> = vec![];
		let mut success_count: usize = 0;
		let mut success_threshold: usize = 0;

		let (status_rx, run_ctl) = RunCtl::new();

		let next_entry = RefCell::new(min(last_entry, cfg.offset));
		let next_batch = || {
			let mut ne = next_entry.borrow_mut();

			let entries_to_fetch = last_entry
				.checked_add(1)
				.ok_or_else(|| Error::arithmetic("moving on from last_entry"))?
				.checked_sub(*ne)
				.ok_or_else(|| Error::arithmetic("calculating entries_to_fetch"))?;

			let batch_size = max(
				MIN_BATCH_SIZE,
				min(
					MAX_BATCH_SIZE,
					div_floor(entries_to_fetch, max_fetchers as u64),
				),
			);

			let range = *ne..=min(
				last_entry,
				(*ne)
					.checked_add(batch_size)
					.ok_or_else(|| Error::arithmetic("calculating next fetch range"))?
					.checked_sub(1)
					.ok_or_else(|| Error::arithmetic("adjusting next fetch range"))?,
			);
			*ne = (*ne)
				.checked_add(batch_size)
				.ok_or_else(|| Error::arithmetic("calculating next_entry"))?;
			Ok(range)
		};

		#[allow(clippy::map_err_ignore)] // The error we map provides no useful information
		for i in 0..max(1, min(cfg.initial_fetchers, max_fetchers)) {
			let fetcher = Fetcher::start(
				i,
				log_url.clone(),
				cfg.user_agent.clone(),
				run_ctl.clone(),
				o.mic()
					.map_err(|_| Error::internal("output thread has already been stopped"))?,
			)?;
			fetcher.ctl().fetch(next_batch()?)?;

			fetchers.push(fetcher);
			success_threshold = success_threshold
				.checked_add(SUCCESS_STEP)
				.ok_or_else(|| Error::arithmetic("advancing success_threshold"))?;
		}

		while {
			let ne = next_entry.borrow();
			*ne
		} <= last_entry
		{
			match status_rx.recv() {
				Ok(FetchStatus::Success) => {
					success_count = success_count
						.checked_add(1)
						.ok_or_else(|| Error::arithmetic("incrementing success_count"))?;
					if success_count > success_threshold && fetchers.len() < max_fetchers {
						log::debug!("Spawning fetch worker {}", fetchers.len());
						success_count = 0;
						success_threshold = success_threshold
							.checked_add(SUCCESS_STEP)
							.ok_or_else(|| Error::arithmetic("advancing success_threshold"))?;
						#[allow(clippy::map_err_ignore)] // This error provides no information
						let new_fetcher = Fetcher::start(
							fetchers.len(),
							log_url.clone(),
							cfg.user_agent.clone(),
							run_ctl.clone(),
							o.mic().map_err(|_| {
								Error::internal("output thread has already been stopped")
							})?,
						)?;
						new_fetcher.ctl().fetch(next_batch()?)?;
						fetchers.push(new_fetcher);
					}
				}
				Ok(FetchStatus::Failure) => success_count = 0,
				Ok(FetchStatus::Complete(n)) => fetchers
					.get(n)
					.ok_or_else(|| {
						Error::internal("received Complete message from non-existent Fetcher #{n}")
					})?
					.ctl()
					.fetch(next_batch()?)?,
				Err(e) => return Err(Error::system("failed to receive status message", e)),
			}
		}

		for (i, f) in fetchers.into_iter().enumerate() {
			if let Err(e) = f.stop() {
				log::warn!("Fetcher {i} crashed: {e}");
			}
		}

		last_entry.saturating_sub(cfg.offset).saturating_add(1)
	};

	o.stop(())
		.map_err(|e| Error::system("failed to stop outputter", e))?;

	Ok(stats)
}
