use std::time::Duration;
use ureq::{Agent, AgentBuilder};

const TIMEOUT: Duration = Duration::from_secs(5);

pub fn web_agent(purpose: &str) -> Agent {
    AgentBuilder::new()
        .timeout_connect(TIMEOUT)
        .timeout_read(TIMEOUT)
        .user_agent(&format!(
            "Aquilum/{} (local-first notes; {purpose})",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
}
