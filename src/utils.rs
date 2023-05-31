use url::Url;

/// Url has some rather irritatingly precise ideas about joining URLs together:
///
/// "Note: a trailing slash is significant. Without it, the last path component is considered to
/// be a “file” name to be removed to get at the “directory” that is used as the base"
///
/// That means that if a user specifies a log URL with a path that doesn't end in a slash, the
/// URLs we produce will be broken.  Rather than try to educate users on the vagaries of URL
/// handling, we'll just add a slash ourselves if necessary.
#[must_use]
pub fn fix_url(mut url: Url) -> Url {
	if !url.path().ends_with('/') {
		url.set_path(&format!("{}/", url.path()));
	};
	url
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fixed_url_mangles_unslashed_base_domain() {
		assert_eq!("https://example.com/", fix_url(Url::parse("https://example.com").unwrap()).to_string());
	}

	#[test]
	fn fixed_url_leaves_slashed_base_domain() {
		assert_eq!("https://example.com/", fix_url(Url::parse("https://example.com/").unwrap()).to_string());
	}

	#[test]
	fn fixed_url_joins_to_base_domain_correctly() {
		assert_eq!("https://example.com/foo/bar/baz", fix_url(Url::parse("https://example.com").unwrap()).join("foo/bar/baz").unwrap().to_string());
	}

	#[test]
	fn fixed_url_joins_to_slashed_base_domain_correctly() {
		assert_eq!("https://example.com/foo/bar/baz", fix_url(Url::parse("https://example.com/").unwrap()).join("foo/bar/baz").unwrap().to_string());
	}

	#[test]
	fn fixed_url_joins_to_pathed_domain_correctly() {
		assert_eq!("https://example.com/foo/bar/baz", fix_url(Url::parse("https://example.com/foo/bar").unwrap()).join("baz").unwrap().to_string());
	}

	#[test]
	fn fixed_url_joins_to_slashed_pathed_domain_correctly() {
		assert_eq!("https://example.com/foo/bar/baz", fix_url(Url::parse("https://example.com/foo/bar/").unwrap()).join("baz").unwrap().to_string());
	}

	#[test]
	fn crate_example() {
		assert_eq!("https://example.net/a/b/c.png", fix_url(Url::parse("https://example.net/a/b/").unwrap()).join("c.png").unwrap().to_string());
	}
}
