use base64::{engine::general_purpose::STANDARD_NO_PAD as b64, Engine as _};
use serde_json::json;
use std::{
	io,
	sync::{Arc, RwLock},
};

#[derive(Clone, Copy, Debug, Default)]
#[doc(hidden)]
#[non_exhaustive]
pub enum StreamFormat {
	#[default]
	JSON,
	#[cfg(feature = "cbor")]
	CBOR,
}

impl std::fmt::Display for StreamFormat {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		match self {
			StreamFormat::JSON => formatter.write_str("json"),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => formatter.write_str("cbor"),
		}
	}
}

#[cfg(feature = "cbor")]
mod cbor;
#[cfg(feature = "cbor")]
use self::cbor::cbor;
#[cfg(feature = "cbor")]
use ciborium_ll::Header as CBORHeader;

impl TryFrom<&str> for StreamFormat {
	type Error = String;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		match s {
			"json" => Ok(Self::JSON),
			#[cfg(feature = "cbor")]
			"cbor" => Ok(Self::CBOR),
			_ => Err(format!("unknown output format {s:?}")),
		}
	}
}

pub(crate) struct StreamingSerializer<'a> {
	format: StreamFormat,
	writer: Arc<RwLock<Box<dyn io::Write + Send + Sync + 'a>>>,
}

impl<'a> StreamingSerializer<'a> {
	pub(crate) fn new(writer: impl io::Write + Send + Sync + 'a, format: StreamFormat) -> Self {
		Self {
			writer: Arc::new(RwLock::new(Box::new(writer))),
			format,
		}
	}

	fn write(&self, o: &[u8]) -> io::Result<()> {
		#[allow(clippy::expect_used)] // At the point this happens, we're *right* fucked
		self.writer
			.write()
			.expect("writer to not be poisoned")
			.write_all(o)
	}

	pub(crate) fn string(&self, s: &str) -> io::Result<()> {
		self.write(&match self.format {
			StreamFormat::JSON => json!(s).to_string().into_bytes(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.text(s, None))?,
		})
	}

	pub(crate) fn bytes(&self, b: &[u8]) -> io::Result<()> {
		self.write(&match self.format {
			StreamFormat::JSON => json!(b64.encode(b)).to_string().into_bytes(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.bytes(b, None))?,
		})
	}

	pub(crate) fn uint(&self, u: u64) -> io::Result<()> {
		self.write(&match self.format {
			StreamFormat::JSON => json!(u).to_string().into_bytes(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.push(CBORHeader::Positive(u)))?,
		})
	}

	pub(crate) fn map(&self) -> io::Result<StreamingMap<'a>> {
		self.write(&match self.format {
			StreamFormat::JSON => b"{".to_vec(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.push(CBORHeader::Map(None)))?,
		})?;
		Ok(StreamingMap {
			s: Self {
				writer: Arc::<RwLock<Box<dyn io::Write + Send + Sync>>>::clone(&self.writer),
				format: self.format,
			},
			element_written: false,
			format: self.format,
		})
	}

	pub(crate) fn seq(&self) -> io::Result<StreamingSeq<'a>> {
		self.write(&match self.format {
			StreamFormat::JSON => b"[".to_vec(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.push(CBORHeader::Array(None)))?,
		})?;

		Ok(StreamingSeq {
			s: Self {
				writer: Arc::<RwLock<Box<dyn io::Write + Send + Sync>>>::clone(&self.writer),
				format: self.format,
			},
			element_written: false,
			format: self.format,
		})
	}
}

pub(crate) struct StreamingMap<'a> {
	format: StreamFormat,
	s: StreamingSerializer<'a>,
	element_written: bool,
}

impl<'a> StreamingMap<'a> {
	pub(crate) fn key(&mut self, key: &str) -> io::Result<()> {
		self.element()?;
		match self.format {
			StreamFormat::JSON => {
				self.s.string(key)?;
				self.s.write(b":")
			}
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => self.s.string(key),
		}
	}

	pub(crate) fn end(&self) -> io::Result<()> {
		self.s.write(&match self.format {
			StreamFormat::JSON => b"}".to_vec(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.push(CBORHeader::Break))?,
		})
	}

	pub(crate) fn string(&self, s: &str) -> io::Result<()> {
		self.s.string(s)
	}

	pub(crate) fn bytes(&self, b: &[u8]) -> io::Result<()> {
		self.s.bytes(b)
	}

	pub(crate) fn uint(&self, u: u64) -> io::Result<()> {
		self.s.uint(u)
	}

	pub(crate) fn map(&self) -> io::Result<StreamingMap<'a>> {
		self.s.map()
	}

	pub(crate) fn seq(&self) -> io::Result<StreamingSeq<'a>> {
		self.s.seq()
	}

	fn element(&mut self) -> io::Result<()> {
		if self.element_written {
			match self.format {
				StreamFormat::JSON => self.s.write(b","),
				#[cfg(feature = "cbor")]
				StreamFormat::CBOR => Ok::<(), io::Error>(()),
			}?;
		}

		self.element_written = true;
		Ok(())
	}
}

pub(crate) struct StreamingSeq<'a> {
	format: StreamFormat,
	s: StreamingSerializer<'a>,
	element_written: bool,
}

impl<'a> StreamingSeq<'a> {
	pub(crate) fn end(&self) -> io::Result<()> {
		self.s.write(&match self.format {
			StreamFormat::JSON => b"]".to_vec(),
			#[cfg(feature = "cbor")]
			StreamFormat::CBOR => cbor(|mut enc| enc.push(CBORHeader::Break))?,
		})
	}

	#[allow(unused)] // At some point, I may need this...
	pub(crate) fn string(&mut self, s: &str) -> io::Result<()> {
		self.element()?;
		self.s.string(s)
	}

	pub(crate) fn bytes(&mut self, b: &[u8]) -> io::Result<()> {
		self.element()?;
		self.s.bytes(b)
	}

	#[allow(unused)] // At some point, I may need this...
	pub(crate) fn uint(&mut self, u: u64) -> io::Result<()> {
		self.element()?;
		self.s.uint(u)
	}

	pub(crate) fn map(&mut self) -> io::Result<StreamingMap<'a>> {
		self.element()?;
		self.s.map()
	}

	#[allow(unused)] // At some point, I may need this...
	pub(crate) fn seq(&mut self) -> io::Result<StreamingSeq<'a>> {
		self.element()?;
		self.s.seq()
	}

	fn element(&mut self) -> io::Result<()> {
		if self.element_written {
			match self.format {
				StreamFormat::JSON => self.s.write(b","),
				#[cfg(feature = "cbor")]
				StreamFormat::CBOR => Ok::<(), io::Error>(()),
			}?;
		}

		self.element_written = true;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	mod json {
		use super::*;

		#[test]
		fn serialize_an_empty_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			s.string("").unwrap();
			drop(s);

			assert_eq!(br#""""#, &buf[..]);
		}

		#[test]
		fn serialize_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			s.string("ohai!").unwrap();
			drop(s);

			assert_eq!(&br#""ohai!""#[..], &buf[..]);
		}

		#[test]
		fn serialize_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			s.bytes(b"ohai!").unwrap();
			drop(s);

			assert_eq!(&br#""b2hhaSE""#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			s.uint(420).unwrap();
			drop(s);

			assert_eq!(&br#"420"#[..], &buf[..]);
		}

		#[test]
		fn serialize_an_empty_seq() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut seq = s.seq().unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(&br#"[]"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut seq = s.seq().unwrap();
			seq.string("ohai!").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(&br#"["ohai!"]"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut seq = s.seq().unwrap();
			seq.bytes(b"ohai!").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(&br#"["b2hhaSE"]"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut seq = s.seq().unwrap();
			seq.uint(420).unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(&br#"[420]"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_bunch_of_stuff() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut seq = s.seq().unwrap();
			seq.string("ohai!").unwrap();
			seq.bytes(b"ohai!").unwrap();
			seq.uint(420).unwrap();
			let mut map = seq.map().unwrap();
			map.key("foo").unwrap();
			map.string("bar").unwrap();
			map.key("baz").unwrap();
			map.bytes(b"\xC2\x89\x9Bj").unwrap();
			map.end().unwrap();
			drop(map);
			seq.string("woot").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(
				&br#"["ohai!","b2hhaSE",420,{"foo":"bar","baz":"wombag"},"woot"]"#[..],
				&buf[..]
			);
		}

		#[test]
		fn serialize_an_empty_map() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut map = s.map().unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(&br#"{}"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut map = s.map().unwrap();
			map.key("string").unwrap();
			map.string("ohai!").unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(&br#"{"string":"ohai!"}"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut map = s.map().unwrap();
			map.key("bytes").unwrap();
			map.bytes(b"ohai!").unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(&br#"{"bytes":"b2hhaSE"}"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut map = s.map().unwrap();
			map.key("uint").unwrap();
			map.uint(420).unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(&br#"{"uint":420}"#[..], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_bunch_of_stuff() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::JSON);
			let mut map = s.map().unwrap();
			map.key("string").unwrap();
			map.string("ohai!").unwrap();
			map.key("bytes").unwrap();
			map.bytes(b"ohai!").unwrap();
			map.key("uint").unwrap();
			map.uint(420).unwrap();
			map.key("seq").unwrap();
			let mut seq = map.seq().unwrap();
			seq.string("one").unwrap();
			seq.bytes(b"\xB7\n(").unwrap();
			seq.uint(3).unwrap();
			seq.end().unwrap();
			drop(seq);
			map.key("four").unwrap();
			map.uint(5).unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(&br#"{"string":"ohai!","bytes":"b2hhaSE","uint":420,"seq":["one","twoo",3],"four":5}"#[..], &buf[..]);
		}
	}

	#[cfg(feature = "cbor")]
	mod cbor {
		use super::*;
		use hex_literal::hex;

		#[test]
		fn serialize_an_empty_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			s.string("").unwrap();
			drop(s);

			assert_eq!(hex!["60"], &buf[..]);
		}

		#[test]
		fn serialize_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			s.string("ohai!").unwrap();
			drop(s);

			assert_eq!(hex!["65 6F68616921"], &buf[..]);
		}

		#[test]
		fn serialize_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			s.bytes(b"ohai!").unwrap();
			drop(s);

			assert_eq!(hex!["45 6F68616921"], &buf[..]);
		}

		#[test]
		fn serialize_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			s.uint(420).unwrap();
			drop(s);

			assert_eq!(hex!["19 01A4"], &buf[..]);
		}

		#[test]
		fn serialize_an_empty_seq() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut seq = s.seq().unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(hex!["9F FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut seq = s.seq().unwrap();
			seq.string("ohai!").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(hex!["9F 65 6F68616921 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut seq = s.seq().unwrap();
			seq.bytes(b"ohai!").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(hex!["9F 45 6F68616921 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut seq = s.seq().unwrap();
			seq.uint(420).unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(hex!["9F 19 01A4 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_seq_with_a_bunch_of_stuff() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut seq = s.seq().unwrap();
			seq.string("ohai!").unwrap();
			seq.bytes(b"ohai!").unwrap();
			seq.uint(420).unwrap();
			let mut map = seq.map().unwrap();
			map.key("foo").unwrap();
			map.string("bar").unwrap();
			map.key("baz").unwrap();
			map.bytes(b"\xC2\x89\x9Bj").unwrap();
			map.end().unwrap();
			drop(map);
			seq.string("woot").unwrap();
			seq.end().unwrap();
			drop(seq);
			drop(s);

			assert_eq!(hex!["9F 65 6F68616921 45 6F68616921 19 01A4 BF 63 666F6F 63 626172 63 62617A 44 C2899B6A FF 64 776F6F74 FF"],
					    &buf[..]);
		}

		#[test]
		fn serialize_an_empty_map() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut map = s.map().unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(hex!["BF FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_string() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut map = s.map().unwrap();
			map.key("string").unwrap();
			map.string("ohai!").unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(hex!["BF 66 737472696E67 65 6F68616921 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_bytes() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut map = s.map().unwrap();
			map.key("bytes").unwrap();
			map.bytes(b"ohai!").unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(hex!["BF 65 6279746573 45 6F68616921 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_uint() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut map = s.map().unwrap();
			map.key("uint").unwrap();
			map.uint(420).unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(hex!["BF 64 75696E74 19 01A4 FF"], &buf[..]);
		}

		#[test]
		fn serialize_a_map_with_a_bunch_of_stuff() {
			let mut buf = vec![];

			let mut s = StreamingSerializer::new(&mut buf, StreamFormat::CBOR);
			let mut map = s.map().unwrap();
			map.key("string").unwrap();
			map.string("ohai!").unwrap();
			map.key("bytes").unwrap();
			map.bytes(b"ohai!").unwrap();
			map.key("uint").unwrap();
			map.uint(420).unwrap();
			map.key("seq").unwrap();
			let mut seq = map.seq().unwrap();
			seq.string("one").unwrap();
			seq.bytes(b"\xB7\n(").unwrap();
			seq.uint(3).unwrap();
			seq.end().unwrap();
			drop(seq);
			map.key("four").unwrap();
			map.uint(5).unwrap();
			map.end().unwrap();
			drop(map);
			drop(s);

			assert_eq!(hex!["BF 66 737472696E67 65 6F68616921 65 6279746573 45 6F68616921 64 75696E74 19 01A4 63 736571 9F 63 6F6E65 43 B70A28 03 FF 64 666F7572 05 FF"], &buf[..]);
		}
	}
}
