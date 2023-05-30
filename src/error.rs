//! Error representations
//!

use std::fmt::Display;

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
#[allow(missing_docs, clippy::missing_docs_in_private_items)] // if the error name and description don't explain it, a one-line comment isn't going to help either
pub enum Error {
	#[error("An internal error occurred: {0} (please report a bug!)")]
	InternalError(String),

	#[error("{0}: {1}")]
	SystemError(String, String),

	#[error("failed to parse JSON for {0}: {1}")]
	JsonParseError(String, String),

	#[error("failed to decode log entry: {0}")]
	EntryDecodingError(String),

	#[error("HTTP request failed: {0}")]
	RequestError(ureq::Error),

	#[error("failed to serialize {0} output: {1}")]
	OutputError(String, String),

	#[error("failed to construct {0} URL: {1}")]
	URLError(String, url::ParseError),

	#[error("arithmetic operation overflowed: {0}")]
	ArithmeticOverflow(String),
}

impl Error {
	pub(crate) fn internal<D>(desc: D) -> Self
	where
		D: Display,
	{
		Self::InternalError(desc.to_string())
	}

	pub(crate) fn system<D, E>(desc: D, e: E) -> Self
	where
		D: Display,
		E: Display,
	{
		Self::SystemError(desc.to_string(), e.to_string())
	}

	pub(crate) fn output<D, E>(desc: D, e: E) -> Self
	where
		D: Display,
		E: Display,
	{
		Self::OutputError(desc.to_string(), e.to_string())
	}

	pub(crate) fn json_parse<C, E>(ctx: C, e: E) -> Self
	where
		C: Display,
		E: Display,
	{
		Self::JsonParseError(ctx.to_string(), e.to_string())
	}

	pub(crate) fn arithmetic<O>(op: O) -> Self
	where
		O: Display,
	{
		Self::ArithmeticOverflow(op.to_string())
	}
}
