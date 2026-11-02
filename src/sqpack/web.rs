//! HTTP Range access on native (Tokio/reqwest) and browser WASM (Fetch).
//!
//! The server must support HEAD with Content-Length and a strong ETag, then
//! single-range GET with 206 and Content-Range. Missing/weak validators are
//! rejected rather than silently mixing versions. Serve unencoded container
//! bytes. Cross-origin browser use requires CORS exposing ETag, Content-Range
//! and Content-Encoding, and allowing If-Match. No directory listing is needed.
//!
//! Accumulated responses are bounded by the requested length, independently of
//! any configurable budget. `SqPack`/`PackReader` offer an optional per-request cap;
//! direct `ReadAt` callers must choose a bounded request length. This does not
//! bound transport buffers or aggregate concurrent requests. Responses ignoring
//! Range are rejected before collecting their bodies. No binary format parsing.

use super::{PackSource, ReadAt, is_valid_file_name};
use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, Response, StatusCode, Url};
use std::collections::TryReserveError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidFileName,
    InvalidBaseUrl,
    Http(reqwest::Error),
    Status(u16),
    Protocol(&'static str),
    Changed,
    InvalidRange,
    Allocation(TryReserveError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFileName => f.write_str("invalid relative container name"),
            Self::InvalidBaseUrl => {
                f.write_str("expected an HTTP(S) root URL without credentials, query or fragment")
            }
            Self::Http(error) => write!(f, "HTTP transport error: {error}"),
            Self::Status(status) => write!(f, "unexpected HTTP status {status}"),
            Self::Protocol(message) => write!(f, "invalid range server response: {message}"),
            Self::Changed => f.write_str("container version changed"),
            Self::InvalidRange => f.write_str("read range overflows or exceeds file size"),
            Self::Allocation(error) => write!(f, "allocation failed: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Allocation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

#[derive(Clone, Debug)]
pub struct Web {
    client: Client,
    root: Url,
}

impl Web {
    /// Build a reusable HTTP client; performs no requests. A trailing slash is
    /// added to the root path if absent. Native operations need a Tokio runtime;
    /// browser operations need a Fetch/ReadableStream-capable environment.
    pub fn new(root: &str) -> Result<Self, Error> {
        let mut root = Url::parse(root).map_err(|_| Error::InvalidBaseUrl)?;
        if !matches!(root.scheme(), "http" | "https")
            || !root.has_host()
            || !root.username().is_empty()
            || root.password().is_some()
            || root.query().is_some()
            || root.fragment().is_some()
        {
            return Err(Error::InvalidBaseUrl);
        }
        if !root.path().ends_with('/') {
            let path = format!("{}/", root.path());
            root.set_path(&path);
        }
        let builder = Client::builder();
        // Disable decoding even if another dependency enables reqwest's codecs:
        // HTTP byte offsets must refer to the bytes we return to the parser.
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .timeout(std::time::Duration::from_secs(30));
        Ok(Self {
            client: builder.build()?,
            root,
        })
    }
}

impl PackSource for Web {
    type Error = Error;
    type File = WebFile;

    async fn open(&self, name: &str) -> Result<Option<WebFile>, Error> {
        if !is_valid_file_name(name) {
            return Err(Error::InvalidFileName);
        }
        let url = self.root.join(name).map_err(|_| Error::InvalidBaseUrl)?;
        let response = identity(self.client.head(url)).send().await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status() != StatusCode::OK {
            return Err(Error::Status(response.status().as_u16()));
        }
        check_encoding(response.headers())?;
        let size = decimal(required(response.headers(), header::CONTENT_LENGTH)?)?;
        let etag = strong_etag(response.headers())?.clone();
        Ok(Some(WebFile {
            client: self.client.clone(),
            // Follow the final metadata URL on subsequent reads, if redirected.
            url: response.url().clone(),
            size,
            etag,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct WebFile {
    client: Client,
    url: Url,
    size: u64,
    etag: HeaderValue,
}

impl ReadAt for WebFile {
    type Error = Error;

    fn len(&self) -> u64 {
        self.size
    }

    async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Error> {
        let count = u64::try_from(length).map_err(|_| Error::InvalidRange)?;
        let end = offset.checked_add(count).ok_or(Error::InvalidRange)?;
        if end > self.size {
            return Err(Error::InvalidRange);
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let request = self
            .client
            .get(self.url.clone())
            .header(header::RANGE, format!("bytes={offset}-{}", end - 1))
            .header(header::IF_MATCH, self.etag.clone());
        let response = identity(request).send().await?;
        validate_range(&response, offset, end, self.size, &self.etag)?;
        collect_exact(response, length).await
    }
}

fn identity(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    // Accept-Encoding is controlled by the browser, not by Fetch callers.
    #[cfg(not(target_arch = "wasm32"))]
    let request = request.header(header::ACCEPT_ENCODING, "identity");
    request
}

fn required(headers: &HeaderMap, name: header::HeaderName) -> Result<&str, Error> {
    let mut values = headers.get_all(name).iter();
    let value = values.next().ok_or(Error::Protocol(
        "missing required header (check CORS exposure)",
    ))?;
    if values.next().is_some() {
        return Err(Error::Protocol("duplicate header"));
    }
    value
        .to_str()
        .map_err(|_| Error::Protocol("invalid header text"))
}

fn decimal(value: &str) -> Result<u64, Error> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Protocol("invalid decimal length or offset"));
    }
    value
        .parse()
        .map_err(|_| Error::Protocol("length or offset overflow"))
}

fn check_encoding(headers: &HeaderMap) -> Result<(), Error> {
    if headers.contains_key(header::CONTENT_ENCODING)
        && !required(headers, header::CONTENT_ENCODING)?.eq_ignore_ascii_case("identity")
    {
        return Err(Error::Protocol("encoded container response"));
    }
    Ok(())
}

fn strong_etag(headers: &HeaderMap) -> Result<&HeaderValue, Error> {
    let mut values = headers.get_all(header::ETAG).iter();
    let value = values.next().ok_or(Error::Protocol(
        "strong ETag required (check CORS exposure)",
    ))?;
    if values.next().is_some() {
        return Err(Error::Protocol("duplicate ETag"));
    }
    let bytes = value.as_bytes();
    if bytes.len() < 2
        || bytes[0] != b'"'
        || bytes[bytes.len() - 1] != b'"'
        || !bytes[1..bytes.len() - 1]
            .iter()
            .all(|&b| b == 0x21 || (0x23..=0x7e).contains(&b) || b >= 0x80)
    {
        return Err(Error::Protocol(
            "strong ETag required; weak or malformed validator",
        ));
    }
    Ok(value)
}

fn validate_range(
    response: &Response,
    start: u64,
    end: u64,
    size: u64,
    etag: &HeaderValue,
) -> Result<(), Error> {
    if response.status() == StatusCode::PRECONDITION_FAILED {
        return Err(Error::Changed);
    }
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(Error::Status(response.status().as_u16()));
    }
    let headers = response.headers();
    check_encoding(headers)?;
    if strong_etag(headers)? != etag {
        return Err(Error::Changed);
    }
    let value = required(headers, header::CONTENT_RANGE)?;
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|v| v.split_once('/'))
        .ok_or(Error::Protocol("invalid Content-Range"))?;
    let (first, last) = range
        .split_once('-')
        .ok_or(Error::Protocol("invalid Content-Range"))?;
    if decimal(first)? != start
        || decimal(last)?.checked_add(1) != Some(end)
        || decimal(total)? != size
    {
        return Err(Error::Protocol(
            "Content-Range does not match requested snapshot range",
        ));
    }
    if headers.contains_key(header::CONTENT_LENGTH)
        && decimal(required(headers, header::CONTENT_LENGTH)?)? != end - start
    {
        return Err(Error::Protocol("unexpected Content-Length"));
    }
    Ok(())
}

async fn collect_exact(response: Response, length: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(Error::Allocation)?;
    let stream = response.bytes_stream();
    let mut stream = std::pin::pin!(stream);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > length - bytes.len() {
            return Err(Error::Protocol("response body exceeds requested range"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() != length {
        return Err(Error::Protocol("short range response body"));
    }
    Ok(bytes)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    // In-memory HTTP messages only: no listener, server, or external requests.
    fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Response {
        let mut builder = http::Response::builder().status(status);
        for &(name, value) in headers {
            builder = builder.header(name, value);
        }
        builder.body(body.to_vec()).unwrap().into()
    }

    fn headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("etag", "\"v1\""),
            ("content-range", "bytes 1-3/8"),
            ("content-length", "3"),
        ]
    }

    fn validate(response: &Response) -> Result<(), Error> {
        validate_range(response, 1, 4, 8, &HeaderValue::from_static("\"v1\""))
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    #[test]
    fn root_urls_are_validated_and_preserve_the_prefix() {
        for url in [
            "/sqpack",
            "file:///sqpack",
            "ftp://host/sqpack",
            "https://host/sqpack?token=a",
            "https://host/#x",
            "https://u:p@host/",
        ] {
            assert!(matches!(Web::new(url), Err(Error::InvalidBaseUrl)));
        }
        let web = Web::new("https://example.invalid/game/sqpack").unwrap();
        assert_eq!(
            web.root.join("synthetic/file.dat").unwrap().as_str(),
            "https://example.invalid/game/sqpack/synthetic/file.dat"
        );
    }

    #[test]
    fn direct_opens_reject_invalid_names_before_http_requests() {
        let source = Web::new("https://example.invalid/sqpack/").unwrap();
        runtime().block_on(async {
            for name in [
                "",
                "/absolute.dat",
                "../escape.dat",
                "a/../b",
                "a\\b",
                "a?b",
                "a%2fb",
                "https://other.invalid/file",
            ] {
                assert!(matches!(
                    source.open(name).await,
                    Err(Error::InvalidFileName)
                ));
            }
        });
    }

    #[test]
    fn exact_partial_response_is_accepted() {
        let response = response(206, &headers(), b"abc");
        validate(&response).unwrap();
        assert_eq!(
            runtime().block_on(collect_exact(response, 3)).unwrap(),
            b"abc"
        );
    }

    #[test]
    fn ignored_ranges_and_http_errors_are_not_absence() {
        for status in [200, 301, 403, 404, 416, 500] {
            assert!(
                matches!(validate(&response(status, &headers(), b"")), Err(Error::Status(actual)) if actual == status)
            );
        }
        assert!(matches!(
            validate(&response(412, &headers(), b"")),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn malformed_mismatched_and_duplicate_headers_are_rejected() {
        for (name, value) in [
            ("content-range", "bytes 0-2/8"),
            ("content-range", "bytes 1-3/9"),
            ("content-range", "bytes */8"),
            ("content-range", "bytes +1-3/8"),
            ("content-range", "bytes 1-18446744073709551615/8"),
            ("content-range", "bytes 1-3/18446744073709551616"),
            ("content-length", "4"),
            ("content-length", "+3"),
        ] {
            let mut hs = headers();
            hs.retain(|(key, _)| *key != name);
            hs.push((name, value));
            assert!(
                validate(&response(206, &hs, b"abc")).is_err(),
                "{name}: {value}"
            );
        }
        for missing in ["etag", "content-range"] {
            let hs: Vec<_> = headers()
                .into_iter()
                .filter(|(key, _)| *key != missing)
                .collect();
            assert!(validate(&response(206, &hs, b"abc")).is_err());
        }
        for duplicate in [
            ("etag", "\"v1\""),
            ("content-range", "bytes 1-3/8"),
            ("content-length", "3"),
        ] {
            let mut hs = headers();
            hs.push(duplicate);
            assert!(validate(&response(206, &hs, b"abc")).is_err());
        }
    }

    #[test]
    fn weak_changed_or_encoded_representations_are_rejected() {
        for etag in ["W/\"v1\"", "v1", "\"v1\", \"v2\"", "\"v2\""] {
            let mut hs = headers();
            hs.retain(|(key, _)| *key != "etag");
            hs.push(("etag", etag));
            assert!(validate(&response(206, &hs, b"abc")).is_err());
        }
        let mut hs = headers();
        hs.push(("content-encoding", "gzip"));
        assert!(validate(&response(206, &hs, b"abc")).is_err());
    }

    #[test]
    fn bounded_collection_rejects_short_and_oversized_bodies() {
        runtime().block_on(async {
            for bytes in [b"ab".as_slice(), b"abcd".as_slice()] {
                assert!(matches!(
                    collect_exact(response(206, &headers(), bytes), 3).await,
                    Err(Error::Protocol(_))
                ));
            }
            let hs: Vec<_> = headers()
                .into_iter()
                .filter(|(key, _)| *key != "content-length")
                .collect();
            let response = response(206, &hs, b"abc");
            validate(&response).unwrap();
            assert_eq!(collect_exact(response, 3).await.unwrap(), b"abc");
        });
    }

    #[test]
    fn empty_or_invalid_reads_do_not_send_requests() {
        let file = WebFile {
            client: Client::new(),
            url: Url::parse("https://example.invalid/file").unwrap(),
            size: 8,
            etag: HeaderValue::from_static("\"v1\""),
        };
        runtime().block_on(async {
            assert_eq!(file.read_at(8, 0).await.unwrap(), b"");
            assert!(matches!(file.read_at(0, 9).await, Err(Error::InvalidRange)));
            assert!(matches!(file.read_at(7, 2).await, Err(Error::InvalidRange)));
            assert!(matches!(file.read_at(9, 0).await, Err(Error::InvalidRange)));
            assert!(matches!(
                file.read_at(u64::MAX, 1).await,
                Err(Error::InvalidRange)
            ));
        });
    }

    #[test]
    fn sqpack_checks_budget_before_http_requests() {
        use crate::sqpack::SqPack;

        // Supply opened metadata without making a HEAD request.
        struct Opened(WebFile);
        impl PackSource for Opened {
            type Error = Error;
            type File = WebFile;
            async fn open(&self, _: &str) -> Result<Option<WebFile>, Error> {
                Ok(Some(self.0.clone()))
            }
        }
        let source = Opened(WebFile {
            client: Client::new(),
            url: Url::parse("https://example.invalid/file").unwrap(),
            size: 8,
            etag: HeaderValue::from_static("\"v1\""),
        });
        runtime().block_on(async {
            let name = "synthetic.dat";
            let sqpack = SqPack::new(source).with_max_read_bytes(Some(4));
            let reader = sqpack.open(name).await.unwrap();
            assert!(matches!(
                reader.read_at(0, 5).await,
                Err(crate::sqpack::Error::ReadTooLarge {
                    requested: 5,
                    limit: 4
                })
            ));
            assert_eq!(reader.read_at(8, 0).await.unwrap(), b"");
        });
    }
}
