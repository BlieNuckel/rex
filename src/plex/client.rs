use std::time::Duration;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use ureq::{Agent, RequestBuilder};

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

    /// Streaming GET for media downloads; returns `Content-Length` and the body reader
    pub fn stream(&self, path: &str) -> Result<(Option<u64>, ureq::BodyReader<'static>)> {
        let resp = self
            .headers(self.stream_agent.get(format!("{}{path}", self.base)), &[])
            .call()
            .with_context(|| format!("GET {}{path}", self.base))?;
        let body = resp.into_body();
        Ok((body.content_length(), body.into_reader()))
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let req = self.headers(self.agent.post(format!("{}{path}", self.base)), query);
        req.send_empty()
            .and_then(|mut r| r.body_mut().read_json())
            .with_context(|| format!("POST {}{path}", self.base))
    }
}
