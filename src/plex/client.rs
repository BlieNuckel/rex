use std::time::Duration;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use ureq::{Agent, RequestBuilder};

pub struct PlexClient {
    agent: Agent,
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
        let device = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|h| h.trim().to_owned())
            .unwrap_or_else(|_| "rex".into());
        Self {
            agent,
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

    pub fn post<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let req = self.headers(self.agent.post(format!("{}{path}", self.base)), query);
        req.send_empty()
            .and_then(|mut r| r.body_mut().read_json())
            .with_context(|| format!("POST {}{path}", self.base))
    }
}
