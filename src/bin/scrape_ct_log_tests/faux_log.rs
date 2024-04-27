//! A mock HTTP server that tries to behave like a CT log
//!

use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
use ct_structs::v1::response::GetSth as GetSthResponse;
use httptest::{matchers, responders, Expectation, ServerHandle, ServerPool};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::future::Future;
use std::ops::{Range, RangeInclusive};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

lazy_static! {
	static ref FAUX_LOG_POOL: ServerPool = ServerPool::new(16);
}

#[derive(Debug)]
pub(crate) struct FauxLog<'a> {
	srv: ServerHandle<'a>,
	tree_head: GetSthResponse,
	entries: HashMap<u64, LogEntry>,
	pub(crate) chunk_size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LogEntry {
	leaf_input: String,
	extra_data: String,
}

impl FauxLog<'_> {
	pub(crate) fn new(expected_entries_requests: Range<usize>) -> Arc<Mutex<FauxLog<'static>>> {
		let sth = GetSthResponse {
			tree_size: 0,
			timestamp: 0,
			sha256_root_hash: Default::default(),
			tree_head_signature: Default::default(),
		};
		let log = Arc::new(Mutex::new(FauxLog {
			srv: FAUX_LOG_POOL.get_server(),
			tree_head: sth,
			entries: Default::default(),
			chunk_size: u64::MAX,
		}));

		{
			let mlog = log.lock().unwrap();

			mlog.srv.expect(FauxLog::sth_expectation(log.clone()));
			mlog.srv.expect(FauxLog::entries_expectation(
				log.clone(),
				expected_entries_requests,
			));
		}

		log
	}

	#[allow(unused)] // This is a method we call when we want to find out why the FauxLog had a sook
	pub(crate) fn poke(&mut self) {
		self.srv.verify_and_clear();
	}

	pub(crate) fn sth(
		&mut self,
		tree_size: u64,
		timestamp: u64,
		sha256_root_hash: Vec<u8>,
		tree_head_signature: Vec<u8>,
	) {
		self.tree_head = GetSthResponse {
			tree_size,
			timestamp,
			sha256_root_hash,
			tree_head_signature,
		};
	}

	pub(crate) fn add_entry(&mut self, id: u64, leaf_input: &[u8], extra_data: &[u8]) {
		self.entries.insert(
			id,
			LogEntry {
				leaf_input: b64.encode(leaf_input),
				extra_data: b64.encode(extra_data),
			},
		);
	}

	pub(crate) fn url(&self) -> String {
		self.srv.url("").to_string()
	}

	fn entry_at(&self, i: u64) -> Option<LogEntry> {
		self.entries.get(&i).cloned()
	}

	fn sth_expectation(log: Arc<Mutex<FauxLog<'static>>>) -> Expectation {
		Self::sth_request()
			.times(1..)
			.respond_with(SthResponder(log))
	}

	fn sth_request() -> httptest::ExpectationBuilder {
		Expectation::matching(matchers::request::method_path("GET", "/ct/v1/get-sth"))
	}

	fn entries_expectation(
		log: Arc<Mutex<FauxLog<'static>>>,
		expected_entries_requests: Range<usize>,
	) -> Expectation {
		Self::entries_request()
			.times(expected_entries_requests)
			.respond_with(EntriesResponder(log))
	}

	fn entries_request() -> httptest::ExpectationBuilder {
		Expectation::matching(matchers::request::method_path("GET", "/ct/v1/get-entries"))
	}
}

async fn _respond(resp: http::Response<hyper::Body>) -> http::Response<hyper::Body> {
	resp
}

struct SthResponder<'a>(Arc<Mutex<FauxLog<'a>>>);

impl responders::Responder for SthResponder<'_> {
	fn respond<'a>(
		&mut self,
		_req: &'a http::Request<bytes::Bytes>,
	) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
		Box::pin(_respond(
			http::Response::builder()
				.status(200)
				.body(
					serde_json::to_string(&self.0.lock().unwrap().tree_head)
						.unwrap()
						.into(),
				)
				.unwrap(),
		))
	}
}

struct EntriesResponder<'a>(Arc<Mutex<FauxLog<'a>>>);

impl responders::Responder for EntriesResponder<'_> {
	fn respond<'a>(
		&mut self,
		req: &'a http::Request<bytes::Bytes>,
	) -> Pin<Box<dyn Future<Output = http::Response<hyper::Body>> + Send + 'a>> {
		let log = self.0.lock().unwrap();

		let mut resp = json!({"entries":[]});

		let mut requested_range = Self::parse_range_from_query(
			req.uri()
				.query()
				.expect("no query string provided to get-entries"),
		);
		if requested_range.end() - requested_range.start() >= log.chunk_size {
			requested_range =
				*requested_range.start()..=(*requested_range.start() + log.chunk_size - 1);
		}

		for i in requested_range {
			if let Some(entry) = log.entry_at(i) {
				resp["entries"]
					.as_array_mut()
					.expect("failed to get resp.entries as array")
					.push(json!(entry));
			}
		}

		Box::pin(_respond(
			http::Response::builder()
				.status(200)
				.body(resp.to_string().into())
				.unwrap(),
		))
	}
}

impl EntriesResponder<'_> {
	fn parse_range_from_query(qs: &str) -> RangeInclusive<u64> {
		let mut start: Option<u64> = None;
		let mut end: Option<u64> = None;

		for m in Regex::new(r"(?P<name>start|end)=(?P<val>[0-9]+)")
			.unwrap()
			.captures_iter(qs)
		{
			if m["name"] == *"start" {
				start = Some(m["val"].parse::<u64>().unwrap());
			} else if m["name"] == *"end" {
				end = Some(m["val"].parse::<u64>().unwrap());
			}
		}

		start.unwrap()..=end.unwrap()
	}
}
