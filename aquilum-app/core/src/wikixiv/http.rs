use crate::web_agent::web_agent;
use std::fmt;
use std::sync::Mutex;
use ureq::Agent;

const PURPOSE: &str = "wikixiv";

pub struct HttpClient {
    agent: Mutex<Agent>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            agent: Mutex::new(web_agent(PURPOSE)),
        }
    }

    pub fn get_text(&self, url: &str) -> Result<String, HttpError> {
        match self.request_once(url) {
            Ok(body) => Ok(body),
            Err(error) if error.is_connection_reset() => {
                self.reset_agent();
                self.request_once(url)
            }
            Err(error) => Err(error),
        }
    }

    fn request_once(&self, url: &str) -> Result<String, HttpError> {
        let agent = self
            .agent
            .lock()
            .map_err(|_| HttpError::Internal("http client lock poisoned".into()))?
            .clone();
        let response = agent.get(url).call().map_err(HttpError::from_ureq)?;
        response
            .into_string()
            .map_err(|error| HttpError::Io(error.to_string()))
    }

    fn reset_agent(&self) {
        if let Ok(mut agent) = self.agent.lock() {
            *agent = web_agent(PURPOSE);
        }
    }
}

#[derive(Debug, Clone)]
pub enum HttpError {
    Offline(String),
    Status(u16, String),
    Io(String),
    Internal(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::Offline(message) => write!(formatter, "offline: {message}"),
            HttpError::Status(code, body) => {
                write!(formatter, "http {code}: {}", truncate_body(body))
            }
            HttpError::Io(message) => write!(formatter, "io: {message}"),
            HttpError::Internal(message) => write!(formatter, "internal: {message}"),
        }
    }
}

fn truncate_body(body: &str) -> String {
    body.chars().take(120).collect()
}

impl HttpError {
    pub fn is_offline(&self) -> bool {
        matches!(self, HttpError::Offline(_))
    }

    fn is_connection_reset(&self) -> bool {
        match self {
            HttpError::Offline(message) | HttpError::Io(message) | HttpError::Internal(message) => {
                let lower = message.to_ascii_lowercase();
                lower.contains("connection reset")
                    || lower.contains("broken pipe")
                    || lower.contains("connection aborted")
                    || lower.contains("forcibly closed")
            }
            HttpError::Status(_, _) => false,
        }
    }

    fn from_ureq(error: ureq::Error) -> Self {
        match error {
            ureq::Error::Status(code, response) => {
                let body = response.into_string().unwrap_or_default();
                HttpError::Status(code, body)
            }
            ureq::Error::Transport(transport) => {
                let message = transport.to_string();
                let lower = message.to_ascii_lowercase();
                if lower.contains("dns")
                    || lower.contains("network")
                    || lower.contains("timed out")
                    || lower.contains("timeout")
                    || lower.contains("connection")
                    || lower.contains("offline")
                    || lower.contains("unreachable")
                {
                    HttpError::Offline(message)
                } else {
                    HttpError::Io(message)
                }
            }
        }
    }
}
