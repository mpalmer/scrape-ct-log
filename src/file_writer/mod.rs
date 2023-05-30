//! Thread that deals with outputting the data that is scraped.
//!
use ct_structs::v1::{ExtraData, SignedEntry, TreeLeafEntry};

use gen_server::{GenServer, Status::Continue};
use url::Url;

use std::io::BufWriter;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{processor, Error};

pub use self::streaming_serializer::StreamFormat as OutputFormat;

#[allow(clippy::result_large_err)] // Oh shoosh
fn current_time() -> Result<u64, Error> {
	#[allow(clippy::expect_used)] // I'll take the risk
	Ok(SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_err(|e| Error::system("we went back in time somehow", e))?
		.as_millis()
		.try_into()
		.expect("wow this code has excellent shelf life"))
}

use std::marker::PhantomData;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Args<W: std::io::Write + Sync + Send> {
	writer: W,
	format: OutputFormat,
	include_chains: bool,
	include_precert_data: bool,
	log_url: Url,

	_m: PhantomData<W>,
}

impl<W: std::io::Write + Sync + Send> Args<W> {
	#[must_use]
	pub fn new(writer: W, log_url: Url) -> Self {
		Args {
			writer,
			log_url,
			format: OutputFormat::JSON,
			include_chains: false,
			include_precert_data: false,
			_m: PhantomData,
		}
	}

	#[must_use]
	pub fn format(mut self, format: OutputFormat) -> Self {
		self.format = format;
		self
	}

	#[must_use]
	pub fn include_chains(mut self, include_chains: bool) -> Self {
		self.include_chains = include_chains;
		self
	}

	#[must_use]
	pub fn include_precert_data(mut self, include_precert_data: bool) -> Self {
		self.include_precert_data = include_precert_data;
		self
	}
}

pub type StopReason = ();

pub struct FileWriter<'a, W: std::io::Write + Sync + Send> {
	map: StreamingMap<'a>,
	entries: Option<StreamingSeq<'a>>,
	include_chains: bool,
	include_precert_data: bool,

	_m: PhantomData<W>,
}

impl<'a, W: std::io::Write + Sync + Send + 'a> std::fmt::Debug for FileWriter<'a, W> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		f.debug_struct("FileWriter").finish()
	}
}

mod streaming_serializer;

use streaming_serializer::{StreamingMap, StreamingSeq, StreamingSerializer};

impl<'a, W: std::io::Write + Sync + Send + 'a> GenServer for FileWriter<'a, W> {
	type Args = Args<W>;
	type Error = Error;
	type Request = processor::Request;
	type StopReason = ();

	fn init(args: Args<W>) -> Result<Self, Self::Error> {
		let ser = StreamingSerializer::new(Box::new(BufWriter::new(args.writer)), args.format);

		let mut map = ser.map().map_err(|e| Error::output("map open", e))?;
		map.key("log_url")
			.map_err(|e| Error::output("log_url key", e))?;
		map.string(args.log_url.as_ref())
			.map_err(|e| Error::output("log_url", e))?;
		map.key("scrape_begin_timestamp")
			.map_err(|e| Error::output("scrape_begin_timestamp key", e))?;
		map.uint(current_time()?)
			.map_err(|e| Error::output("scrape_begin_timestamp", e))?;

		Ok(Self {
			map,
			entries: None,
			include_chains: args.include_chains,
			include_precert_data: args.include_precert_data,
			_m: PhantomData,
		})
	}

	#[allow(clippy::too_many_lines)] // TODO: refactor
	fn handle_cast(
		&mut self,
		request: Self::Request,
	) -> Result<gen_server::Status<Self>, Self::Error> {
		match request {
			processor::Request::Metadata(sth) => {
				self.map
					.key("sth")
					.map_err(|e| Error::output("sth key", e))?;
				let mut sth_map = self
					.map
					.map()
					.map_err(|e| Error::output("sth map open", e))?;
				sth_map
					.key("tree_size")
					.map_err(|e| Error::output("tree_size key", e))?;
				sth_map
					.uint(sth.tree_size)
					.map_err(|e| Error::output("tree_size", e))?;
				sth_map
					.key("timestamp")
					.map_err(|e| Error::output("timestamp key", e))?;
				sth_map
					.uint(sth.timestamp)
					.map_err(|e| Error::output("timestamp", e))?;
				sth_map
					.key("sha256_root_hash")
					.map_err(|e| Error::output("sha256_root_hash key", e))?;
				sth_map
					.bytes(&sth.sha256_root_hash)
					.map_err(|e| Error::output("sha256_root_hash", e))?;
				sth_map
					.key("tree_head_signature")
					.map_err(|e| Error::output("tree_head_signature key", e))?;
				sth_map
					.bytes(&sth.tree_head_signature)
					.map_err(|e| Error::output("tree_head_signature", e))?;
				sth_map
					.end()
					.map_err(|e| Error::output("sth map close", e))?;

				Ok(Continue)
			}
			processor::Request::Entry(id, entry) => {
				if self.entries.is_none() {
					self.map
						.key("entries")
						.map_err(|e| Error::output("entries key", e))?;
					let entries = self
						.map
						.seq()
						.map_err(|e| Error::output("entries open", e))?;
					self.entries = Some(entries);
				}

				if let Some(entries) = &mut self.entries {
					let mut map = entries
						.map()
						.map_err(|e| Error::output("entry map open", e))?;

					let (timestamp, certificate, chain_certs, precert) =
						if let TreeLeafEntry::TimestampedEntry(ts_entry) = &entry.leaf_input.entry {
							match (&ts_entry.signed_entry, &entry.extra_data) {
							 (SignedEntry::X509Entry(x509_entry), ExtraData::X509ExtraData(extra_data)) => Ok((ts_entry.timestamp, &x509_entry.certificate, &extra_data.certificate_chain, None)),
							 (SignedEntry::PrecertEntry(precert_entry), ExtraData::PrecertExtraData(extra_data)) => Ok((ts_entry.timestamp, &extra_data.pre_certificate.certificate, &extra_data.precertificate_chain, Some(precert_entry.clone()))),
							 _ => Err(Error::InternalError(format!("incompatible combination of signed_entry and extra_data ({:?} vs {:?})", ts_entry.signed_entry, entry.extra_data))),
						 }
						} else {
							Err(Error::EntryDecodingError(
								"leaf_input was not a TimestampedEntry".to_string(),
							))
						}?;

					map.key("entry_number")
						.map_err(|e| Error::output("entry_number key", e))?;
					map.uint(id).map_err(|e| Error::output("entry_number", e))?;
					map.key("timestamp")
						.map_err(|e| Error::output("timestamp key", e))?;
					map.uint(timestamp)
						.map_err(|e| Error::output("timestamp", e))?;
					map.key("certificate")
						.map_err(|e| Error::output("certificate key", e))?;
					map.bytes(certificate)
						.map_err(|e| Error::output("certificate", e))?;

					if self.include_chains {
						map.key("chain")
							.map_err(|e| Error::output("chain key", e))?;
						let mut chain = map.seq().map_err(|e| Error::output("chain open", e))?;

						for c in chain_certs {
							chain
								.bytes(&c.certificate)
								.map_err(|e| Error::output("chain entry", e))?;
						}

						chain.end().map_err(|e| Error::output("chain close", e))?;
					}

					if self.include_precert_data {
						if let Some(precert) = precert {
							map.key("precert")
								.map_err(|e| Error::output("precert key", e))?;
							let mut precert_map =
								map.map().map_err(|e| Error::output("precert open", e))?;

							precert_map
								.key("issuer_key_hash")
								.map_err(|e| Error::output("issuer_key_hash key", e))?;
							precert_map
								.bytes(&precert.issuer_key_hash)
								.map_err(|e| Error::output("issuer_key_hash", e))?;
							precert_map
								.key("tbs_certificate")
								.map_err(|e| Error::output("tbs_certificate key", e))?;
							precert_map
								.bytes(&precert.tbs_certificate)
								.map_err(|e| Error::output("tbs_certificate", e))?;

							precert_map
								.end()
								.map_err(|e| Error::output("precert close", e))?;
						}
					}

					map.end().map_err(|e| Error::output("entry map close", e))?;
				}

				Ok(Continue)
			}
		}
	}

	fn terminate(&mut self, _reason: Result<(), Error>) {
		if let Some(ref mut entries) = &mut self.entries {
			drop(entries.end().map_err(|e| Error::output("entries close", e)));
		}
		if let Ok(time) = current_time() {
			drop(
				self.map
					.key("scrape_end_timestamp")
					.map_err(|e| Error::output("scrape_end_timestamp key", e)),
			);
			drop(
				self.map
					.uint(time)
					.map_err(|e| Error::output("scrape_end_timestamp", e)),
			);
		}
		drop(self.map.end().map_err(|e| Error::output("map close", e)));
	}
}
