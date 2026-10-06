use std::time::Duration;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use ureq::{Agent, RequestBuilder};

pub struct Stream {
    /// offset of the first body byte; 0 when the server ignored the range
    pub start: u64,
    /// total size of the resource, if the server said
    pub len: Option<u64>,
    pub body: ureq::BodyReader<'static>,
}

pub struct PlexClient {
    agent: Agent,
    /// kept separate from `agent` so media downloads aren't cut off by its global timeout,
    /// and long-lived so its connection pool skips a TCP/TLS handshake per track
    stream_agent: Agent,
    pub base: String,
    token: String,
    client_id: String,
    device: String,
}

impl PlexClient {
    pub fn new(base: &str, token: &str, client_id: &str, timeout: Duration) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .into();
        // no per-read timeout, so a stalled download blocks its fetch thread until the
        // source is dropped; add a read watchdog if that shows up in practice
        let stream_agent = Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .build()
            .into();
        let device = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|h| h.trim().to_owned())
            .unwrap_or_else(|_| "rex".into());
        Self {
            agent,
            stream_agent,
            base: base.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            client_id: client_id.to_owned(),
            device,
        }
    }

    fn headers<B>(&self, mut r: RequestBuilder<B>, query: &[(&str, &str)]) -> RequestBuilder<B> {
        r = r
            .header("Accept", "application/json")
            .header("X-Plex-Product", "rex")
            .header("X-Plex-Version", env!("CARGO_PKG_VERSION"))
            .header("X-Plex-Client-Identifier", &self.client_id)
            .header("X-Plex-Platform", "Linux")
            .header("X-Plex-Device-Name", &self.device);
        if !self.token.is_empty() {
            r = r.header("X-Plex-Token", &self.token);
        }
        for (k, v) in query {
            r = r.query(*k, *v);
        }
        r
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let req = self.headers(self.agent.get(format!("{}{path}", self.base)), query);
        req.call()
            .and_then(|mut r| r.body_mut().read_json())
            .with_context(|| format!("GET {}{path}", self.base))
    }

    pub fn get_range<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        start: u32,
        size: u32,
    ) -> Result<T> {
        let req = self
            .headers(self.agent.get(format!("{}{path}", self.base)), query)
            .header("X-Plex-Container-Start", start.to_string())
            .header("X-Plex-Container-Size", size.to_string());
        req.call()
            .and_then(|mut r| r.body_mut().read_json())
            .with_context(|| format!("GET {}{path} [{start}+{size}]", self.base))
    }

    /// streaming GET for media downloads starting at byte `from`
    pub fn stream(&self, path: &str, from: u64) -> Result<Stream> {
        let mut req = self.headers(self.stream_agent.get(format!("{}{path}", self.base)), &[]);
        if from > 0 {
            req = req.header("Range", format!("bytes={from}-"));
        }
        let resp = req
            .call()
            .with_context(|| format!("GET {}{path} from {from}", self.base))?;
        let partial = resp.status().as_u16() == 206;
        // "bytes 100-199/1000" -> 1000
        let total = resp
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|t| t.parse().ok());
        let body = resp.into_body();
        let start = if partial { from } else { 0 };
        Ok(Stream {
            start,
            len: total.or_else(|| body.content_length().map(|n| start + n)),
            body: body.into_reader(),
        })
    }

    /// fire-and-forget GET for endpoints whose response body we don't need
    pub fn send(&self, path: &str, query: &[(&str, &str)], timeout: Duration) -> Result<()> {
        self.headers(self.agent.get(format!("{}{path}", self.base)), query)
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .and_then(|r| r.into_body().read_to_vec())
            .with_context(|| format!("GET {}{path}", self.base))?;
        Ok(())
    }

    /// absolute URL for `path` with the token in the query, for consumers that can't send headers
    pub fn url_with_token(&self, path: &str) -> String {
        let sep = if path.contains('?') { '&' } else { '?' };
        format!("{}{path}{sep}X-Plex-Token={}", self.base, self.token)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let req = self.headers(self.agent.post(format!("{}{path}", self.base)), query);
        req.send_empty()
            .and_then(|mut r| r.body_mut().read_json())
            .with_context(|| format!("POST {}{path}", self.base))
    }
}
