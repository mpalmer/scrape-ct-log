//! Workers that actually get the entries from the log.
//!

use ct_structs::v1::response::GetEntries as GetEntriesResponse;

use rand::{thread_rng, Rng};
use std::cmp::min;
use std::ops::RangeInclusive;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use url::Url;

use crate::{error::Error, processor, runner::RunCtl};

const MIN_RETRY_DELAY_MILLIS: u32 = 100;
const MAX_RETRY_DELAY_MILLIS: u32 = 15000;
const BASE_RETRY_SCALING_MILLIS: u32 = 50;

#[derive(Clone, Debug)]
pub(crate) enum FetchStatus {
	Success,
	Failure,
	Complete(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct FetchCtl {
	tx: mpsc::Sender<Cmd>,
}

impl FetchCtl {
	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn fetch(&self, r: RangeInclusive<u64>) -> Result<(), Error> {
		log::debug!("telling Fetcher to fetch {r:?}");
		self.tx
			.send(Cmd::FetchRange(r))
			.map_err(|e| Error::system("failed to send FetchRange to fetch worker", e))
	}
}

#[derive(Debug)]
pub(crate) struct Fetcher {
	h: Option<thread::JoinHandle<Result<(), Error>>>,
	c: FetchCtl,
}

#[derive(Debug)]
enum Cmd {
	FetchRange(RangeInclusive<u64>),
	Stop,
}

impl Fetcher {
	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn start(
		n: usize,
		log_url: Url,
		user_agent: String,
		status: RunCtl,
		processor: processor::Mic,
	) -> Result<Self, Error> {
		let (tx, rx) = mpsc::channel();

		Ok(Fetcher {
			h: Some(
				thread::Builder::new()
					.name(format!("Fetcher{n}"))
					.spawn(move || Self::run(n, &rx, &log_url, &user_agent, &status, &processor))
					.map_err(|e| Error::system("failed to spawn Fetcher thread", e))?,
			),
			c: FetchCtl { tx },
		})
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	pub(crate) fn stop(mut self) -> Result<(), Error> {
		if let Some(h) = self.h {
			log::debug!("signalling Fetcher to stop");
			if let Err(e) = self.c.tx.send(Cmd::Stop) {
				return Err(Error::system(
					"failed to send stop command to fetch worker",
					e,
				));
			}

			self.h = None;
			match h.join() {
				Ok(rv) => rv,
				Err(e_ref) => {
					if let Some(e) = e_ref.downcast_ref::<String>() {
						Err(Error::system("fetch worker thread panicked", e))
					} else {
						Err(Error::system(
							"fetch worker thread panicked",
							"(can't show error because it was not a String)",
						))
					}
				}
			}
		} else {
			Err(Error::InternalError(
				"called stop() on fetch worker when it was already stopped".to_string(),
			))
		}
	}

	pub(crate) fn ctl(&self) -> FetchCtl {
		self.c.clone()
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	fn run(
		n: usize,
		rx: &mpsc::Receiver<Cmd>,
		log_url: &Url,
		user_agent: &str,
		status: &RunCtl,
		processor: &processor::Mic,
	) -> Result<(), Error> {
		log::debug!("Fetcher::run({log_url:?})");
		let http_client = ureq::AgentBuilder::new().user_agent(user_agent).build();
		let entries_url = log_url
			.join("ct/v1/get-entries")
			.map_err(|e| Error::system("failed to construct get-entries URL", e))?;

		loop {
			let cmd = rx.recv();
			log::debug!("received {cmd:?}");
			match cmd {
				Ok(Cmd::Stop) => return Ok(()),
				Ok(Cmd::FetchRange(range)) => {
					if let Err(e) =
						Self::fetch_range(&http_client, &entries_url, range, status, processor)
					{
						log::error!("{}", e);
					} else {
						status.complete(n)?;
					}
				}
				//Ok(u @ _) => return Err(Error::InternalError(format!("unexpected command received in Fetch::run(): {u:?}"))),
				Err(e) => return Err(Error::system("rx.recv() in Fetch::run() returned error", e)),
			}
		}
	}

	#[allow(clippy::result_large_err)] // Oh shoosh
	fn fetch_range(
		client: &ureq::Agent,
		entries_url: &Url,
		mut range: RangeInclusive<u64>,
		status: &RunCtl,
		processor: &processor::Mic,
	) -> Result<(), Error> {
		log::debug!("Fetching entries {range:?} from {entries_url}");
		let mut retry_delay_millis = MIN_RETRY_DELAY_MILLIS;
		let mut retry_delay_scaling_millis = BASE_RETRY_SCALING_MILLIS;

		while range.start() <= range.end() {
			log::debug!("Requesting {entries_url}, {range:?}");

			let response = match client
				.get(entries_url.as_ref())
				.query("start", &format!("{}", range.start()))
				.query("end", &format!("{}", range.end()))
				.call()
			{
				Ok(response) => {
					let result: GetEntriesResponse =
						serde_json::from_reader(response.into_reader())
							.map_err(|e| Error::json_parse("get-entries response", e))?;
					status.success()?;
					retry_delay_millis = MIN_RETRY_DELAY_MILLIS;
					retry_delay_scaling_millis = BASE_RETRY_SCALING_MILLIS;
					result
				}
				Err(ureq::Error::Status(429, _response)) => {
					log::info!("Got told we're doing too many requests");
					status.failure()?;
					let snooze_time_millis = retry_delay_millis
						.checked_add(
							thread_rng()
								.gen::<u32>()
								.rem_euclid(retry_delay_scaling_millis),
						)
						.ok_or_else(|| Error::arithmetic("calculating snooze_time_millis"))?;
					thread::sleep(Duration::from_millis(snooze_time_millis.into()));
					retry_delay_millis = min(
						retry_delay_millis
							.checked_mul(2)
							.ok_or_else(|| Error::arithmetic("doubling retry_delay_millis"))?,
						MAX_RETRY_DELAY_MILLIS,
					);
					retry_delay_scaling_millis = retry_delay_scaling_millis
						.checked_add(BASE_RETRY_SCALING_MILLIS)
						.ok_or_else(|| {
							Error::arithmetic("increasing retry_delay_scaling_millis")
						})?;
					continue;
				}
				Err(ureq::Error::Status(code, response)) if (400..=499).contains(&code) => {
					log::warn!("we sent a response that the server didn't understand.  Server returned HTTP {code}, {:?}", response.into_string().map_err(|e| Error::system("failed to read HTTP response body", e))?);
					continue;
				}
				Err(ureq::Error::Status(code, response)) if code >= 500 => {
					log::info!(
						"HTTP server error {code}: {:?}",
						response
							.into_string()
							.map_err(|e| Error::system("failed to read HTTP response body", e))?
					);
					status.failure()?;
					thread::sleep(Duration::from_secs(1));
					continue;
				}
				Err(e) => return Err(Error::RequestError(e)),
			};

			log::debug!("Received {} entries", response.entries.len());
			log::info!("Fetched entries {range:?}");

			let len = response.entries.len() as u64;
			#[allow(clippy::reversed_empty_ranges)] // An empty range is what I want here
			if len == 0 {
				log::warn!("received no entries fetching {range:?}; possible log misbehaviour");
				range = 1..=0;
			} else {
				for (e, i) in response.entries.into_iter().zip(0u64..) {
					log::debug!(
						"Sending entry {} ({i} of this request) to processor",
						range
							.start()
							.checked_add(i)
							.ok_or_else(|| Error::arithmetic(
								"calculating in-fetch ID (SHOULDN'T HAPPEN)"
							))?
					);
					log::trace!("{e:?}");
					processor.cast(processor::Request::Entry(
						range.start().checked_add(i).ok_or_else(|| {
							Error::arithmetic("calculating absolute entry ID (SHOULDN'T HAPPEN)")
						})?,
						e,
					));
				}
				range =
					(range.start().checked_add(len).ok_or_else(|| {
						Error::arithmetic("calculating start of next fetch range")
					})?)..=*range.end();
			}
		}

		Ok(())
	}
}
