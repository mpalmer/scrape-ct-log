	// Url has some rather irritatingly precise ideas about joining URLs together:
	//
	// "Note: a trailing slash is significant. Without it, the last path component is considered to
	// be a “file” name to be removed to get at the “directory” that is used as the base"
	//
	// That means that if a user specifies a log URL with a path that doesn't end in a slash, the
	// URLs we produce will be broken.  Rather than try to educate users on the vagaries of URL
	// handling, we'll just add a slash ourselves if necessary.
	let base_url = if cfg.log_url.path().chars().last() != Some('/') {
		let mut tmp_url = cfg.log_url.clone();
		tmp_url.set_path(&format!("{}/", cfg.log_url.path()));
		tmp_url
	} else {
		cfg.log_url.clone()
	};

