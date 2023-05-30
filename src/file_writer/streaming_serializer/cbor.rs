use ciborium_ll::Encoder;

pub(crate) fn cbor<F: FnOnce(Encoder<&mut Vec<u8>>) -> std::io::Result<()>>(
	f: F,
) -> std::io::Result<Vec<u8>> {
	let mut v: Vec<u8> = vec![];

	f(Encoder::from(&mut v))?;
	Ok(v)
}
