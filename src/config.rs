use std::time::Duration;

pub struct Config {
    pub repo_owner: String,
    pub repo_name: String,
    pub poll_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_owner: "bitcoin".to_string(),
            repo_name: "bitcoin".to_string(),
            poll_interval: Duration::from_secs(60),
        }
    }
}
