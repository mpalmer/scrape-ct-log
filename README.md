This is `scrape-ct-log`, a tool to quickly and reliably download entries from a [Certificate Transparency](https://certificate-transparency.org) [log](https://certificate.transparency.dev/logs/) into a file for further processing.

# Installation

## Current Status

Because of needed patches to some of our (transitive) dependencies, the only way to install this tool at present is to build it yourself.
This isn't too painful; it just involves [installing a Rust toolchain](https://www.rust-lang.org/learn/get-started), cloning this repo, and running `cargo build --release`.
You'll end up with a binary at `target/release/scrape-ct-log`.


## Intended End Result

(This is how things will be once all our deps are fixed)

Pre-built binaries for the most commonly-used platforms are available from [the project's releases page](https://github.com/mpalmer/scrape-ct-log/releases).
If you'd like a binary for a different platform, please [let me know](https://github.com/mpalmer/scrape-ct-log/issues/new) and I'll see what can be worked out.

Alternately, you can install via `cargo` with `cargo install scrape-ct-log`.

Finally, if you're the sturdy, self-reliant type, you can clone [the repo](https://github.com/mpalmer/scrape-ct-log) and build it yourself with `cargo build --release`.


# Usage

In its most basic form, you call `scrape-ct-log` with the base URL of the CT log you wish to scrape, and it'll output all the (pre-)certificates to stdout [in JSON, with some leading metadata](#structure-of-the-output).

Example:

```sh
# Using crucible because it's a well-known, relatively small test log
scrape-ct-log https://ct.googleapis.com/logs/crucible/
# Prepare for a *massive* pile of JSON
```

Since dumping the better part of 800 million or so certificates (the size of a typical production CT log) to stdout is rarely a useful use-case, you'll typically want to use one or more of the following options to keep things under control.


## Limit the number of entries retrieved

Most often, you'll want to download a "chunk" of entries at once, to store or process.
This is achieved with the `-n` (aka `--number-of-entries`) option.
It takes a positive integer, which is the *maximum* number of entries that will be downloaded.

Example:

```sh
# Download no more than 1,000 entries
scrapt-ct-log -n 1000 https://ct.googleapis.com/logs/crucible/
```

## Start scraping at a given offset

If you've already got entries, and you'd just like the new ones, you can specify the offset to start from with the `-s` (aka `--start-from`) option.
It takes a non-negative integer, which is the first entry number to be downloaded.
Bear in mind that CT log entries are numbered starting from `0`, so `-s 0` is just "start from the beginning", while `-s 1` is "skip the first entry, start with the second".

Example:

```sh
# Start downloading all entries starting from number 420
scrape-ct-log -s 420 https://ct.googleapis.com/logs/crucible/
```

You can, of course, combine this with `-n` to get a kind of "chunking" (useful if you want to download from multiple IP addresses to get around per-IP rate limits):

```sh
# Download ten thousand entries starting from entry number 1,000,000
scrape-ct-log -s 1000000 -n 10000 https://ct.googleapis.com/logs/crucible/
```

## Write to a file

While sending the output to stdout is often fine, it's useful to be able to specify a file to write to with the `-o` (aka `--output`) option.
Note that if you specify an existing file, its contents will be wiped.

Example:

```sh
# Write to /tmp/log_entries
scrape-ct-log -o /tmp/log_entries https://ct.googleapis.com/logs/crucible/
```


## Control the output format

By default, the output is in JSON format, as that is a reasonably well-understood, human-friendly(ish) data format.
However, it's somewhat CPU intensive to parse, and also somewhat verbose when representing binary data (which X.509 certificates are).
Hence, we also support writing out the same information in [CBOR](https://cbor.io/), which is a binary-based format that can be written and read more efficiently, for our purposes.
For this purpose, you can use the `-f` (aka `--format`) option.

Example:

```sh
# Write data to stdout using CBOR
scrape-ct-log -f cbor https://ct.googleapis.com/logs/crucible/
```

As an aside, if you'd like output in a different format, please [let me know](https://github.com/mpalmer/scrape-ct-log/issues/new).
While we can't use [serde](https://crates.io/crates/serde), because of its lack of streaming support for subelements, I'm willing to write a custom encoder for other formats if there's demand (and it can support indefinite length sequences).


## Include certificate chains

In addition to the certificates themselves, CT logs also record (and return) the *chain* of intermediate CA certificates that were included in the submission.
This data is rarely of interest to those analysing certificates, so it is not included by default.
However, if it is of interest to your specific case, you can request that the entry chains be included [in the output](#structure-of-the-output) with the `--include-chains` option.

Example:

```sh
# Write entry chains as well as the certificates themselves
scrape-ct-log --include-chains https://ct.googleapis.com/logs/crucible/
```

Bear in mind that this will make the amount of data output increase *even more* than it would have otherwise.


## Include precertificate data

Precertificates in CT are represented in a slightly odd manner.
The log entry includes both an X.509 certificate that is *almost* the same as the to-be-issued certificate, as well as a separate data blob that is a subset of a certificate, along with the hash of the issuer key.
For *most* purposes, the X.509 certificate is sufficient, and the other data is an unnecessary duplication of information.
If your purpose for using `scrape-ct-log` is one of the few purposes that really does need the other data, you can use the `--include-precert-data` option, and the precert data will show up in the `precertificate` field of each entry that represents a precertificate.

Example:

```sh
# Write tbsCertificate and issuer_key_hash as well
scrape-ct-log --include-precert-data https://ct.googleapis.com/logs/crucible/
```


## Getting more info about what's happening

If you're curious about what's going on, or you think something is going wrong, you can ask for *verbose* output with `-v` (aka `--verbose`).
You can specify this option more times (up to four) and the amount of verbosity will gradually increase.
All verbose output will be sent to stderr (which you can redirect with `2>/tmp/some_file`).

Be aware that this can potentially produce a *lot* of output, especially in the higher verbosity levels.


## Halp!

Like all good CLI tools, `scrape-ct-log` will give you usage information if you provide the `-h` (aka `--help`) option.

Example:

```sh
# Halp!
scrape-ct-log -h
```


# Structure of the output

The output produced by `scrape-ct-log` is a serialized structure of metadata and certificate entries.
Regardless of the actual *format* being used (JSON, MsgPack, etc), it will always have this same structure.

Every field has its data type listed after the field name.
The "basic" types are defined below, while structured subtypes (indicated with angle brackets around the name, such as `<entry>`) have their own definition in a subsection below.
A list of arbitrary length of a given type is indicated as `[type]`, such as `[<entry>]` meaning "a list of `<entry>` structures".

* `string` -- a UTF-8 string.

* `integer` -- an integral number, positive, negative, or zero.
    No specific range of integers is expressed or implied.
    The valid range of the number may be constrained by the serialisation format, your programming language, or the phase of the moon.
    At the very least, be prepared to accept anything representable by a 64 bit twos-complement integer.

* `bytes` -- a sequence of arbitrary octets.
    If the format supports some sort of "binary sequence" type, it will be used directly.
    The formats that support direct encoding are:

    * `msgpack`

    For formats that don't directly support binary sequences, the data will be encoded into a UTF-8 string using base64 with the [RFC3548 alphabet](https://datatracker.ietf.org/doc/html/rfc3548#section-3) and no trailing padding (the pointless `=` characters).
    The formats that will produce base64-encoded `bytes` are:

    * `json`


## Top-level structure

At the top level, the output will consist of a "map" (object, dictionary, associative array, hash, what-have-you) with the following keys:

* `log_url` (`string`) -- simply the URL that was provided.
    Kept in here just in case you have lots of output files laying around, and would like to know where they all came from.

* `scrape_begin_timestamp` (`integer`) -- the number of milliseconds since the Unix epoch at which the program was started.

* `scrape_end_timestamp` (`integer`) -- the number of milliseconds since the Unix epoch at which the program finished.
    Unsurprisingly, subtracting `scrape_begin_timestamp` from this value will give you a pretty good idea of how long the scrape took.

* `sth` (`<sth>`) -- The Signed Tree Head that was presented by the server when we started the scrape.

* `entries` (`[<entry>]`) The set of entries that were retrieved during the scrape.
    Note that the entries may not be in the order that they are in the log, which is why each `<entry>` has the log's `entry_number` encoded in it.


## `<sth>`

The Signed Tree Head is a Certificate Transparency data structure giving you information about the state of the log at a given time.
Our structure is a straight-up clone of the format described in the spec; for more details on the fields, consult [the RFC](https://datatracker.ietf.org/doc/html/rfc6962#section-3.5).

* `tree_size` (`integer`)

* `timestamp` (`integer`)

* `sha256_root_hash` (`bytes`)

* `tree_head_signature` (`bytes`)


## `<entry>`

This is the meat of the whole endeavour -- the log entries themselves.
Fields that are not relevant to a particular entry will not be present.

* `entry_number` (`integer`) -- where in the log this particular entry was found.

* `timestamp` (`integer`) -- the number of *milliseconds* since the epoch at which the entry was submitted, or attested to, or whatever.

* `certificate` (`bytes`) -- the DER-encoded X.509 certificate that is included in the entry, either the issued certificate or the "poisoned" certificate that stands in for the final certificate, in the case of a precertificate.

* `precertificate` (`<precert>`) -- the precertificate data, if the entry is a precertificate, and the `--include-precert-data` has been specified.

* `chain` ([`bytes`]) -- the set of DER-encoded certificates that were submitted to the log along with the entry.
    Only present if the `--include-chains` option was provided.


## `<precert>`

Precertificates are handled in a slightly weird manner in CT, in that they're included in two different forms in the log entry -- both an actual X.509 certificate with a "poison" extension, as well as the [`tbsCertificate`](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.1) of the to-be-issued certificate along with the hash of the issuer key.
The former is included in the main `<entry>` structure, while this structure holds the latter.

* `issuer_key_hash` (`bytes`) -- the SHA-256 hash of the SPKI of the key which was declared to issue the final certificate.

* `tbs_certificate` (`bytes`) -- the DER-encoded `tbsCertificate` structure of the to-be-issued certificate.


# How it works

The goals of `scrape-ct-log` are:

* **Reliability**: if it completes successfully, you've definitely got what you asked for.

* **Fast**: the amount of wall-clock time needed to retrieve a set of log entries should be as short as possible, as long as it doesn't compromise reliability.

* **Efficient**: the load on CT logs, and memory/CPU required, should be minimised, as long as it doesn't compromise speed or reliability.

Towards these goals, this is (roughly) how `scrape-ct-log` does its job.

The set of entries that are to be downloaded is split into reasonably-sized "chunks" (by default, it's calculated based on the number of entries available to be downloaded, but it can be adjusted with `--chunk-size N` if you think you know better).
Each of these chunks is farmed out to a worker thread, which makes repeated requests to the log server asking for the entries in that chunk.
Once the worker thread has completed a chunk, it signals completion to the manager thread, which assigns a new chunk, or tells the thread to shut down if there are no more chunks to download.

Since CT logs typically only return a few entries per request, the worker thread will gather the entries in slow dribbles, gradually working through the entries in its current chunk.
As entries come in, they're fed to a dedicated "output" thread which does the work of writing the serialized certificates to the destination.

Since CT logs will typically allow multiple downloads from a single IP address, we spawn multiple worker threads to download entries in parallel.
We try to adapt the number of workers to the capacity of a log, by gradually "ramping up" the number of workers.
The manager starts out with one worker thread, and as workers successfully complete requests, new worker threads are spawned.
If a worker receives an error response to a request, it pauses for a short period, and tells the manager thread that the log is at capacity, which throttles the manager from spawning any more workers for a while.


# Licence

Unless otherwise stated, everything in this repo is covered by the following
copyright notice:

    Copyright (C) 2023  Matt Palmer <matt@hezmatt.org>

    This program is free software: you can redistribute it and/or modify it
    under the terms of the GNU General Public License version 3, as
    published by the Free Software Foundation.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.

