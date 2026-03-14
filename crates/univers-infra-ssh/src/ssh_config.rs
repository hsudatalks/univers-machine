use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct InlineIdentity {
    pub label: String,
    pub secret: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedEndpoint {
    pub alias: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub identity_files: Vec<PathBuf>,
    pub inline_identities: Vec<InlineIdentity>,
    pub known_hosts_path: Option<PathBuf>,
    pub known_hosts_host: Option<String>,
    pub accept_new_host_keys: bool,
}

impl ResolvedEndpoint {
    pub fn new(
        alias: impl Into<String>,
        host: impl Into<String>,
        user: impl Into<String>,
        port: u16,
        identity_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            alias: alias.into(),
            host: host.into(),
            user: user.into(),
            port,
            identity_files,
            inline_identities: Vec::new(),
            known_hosts_path: None,
            known_hosts_host: None,
            accept_new_host_keys: false,
        }
    }

    pub fn identity_files(&self) -> &[PathBuf] {
        &self.identity_files
    }

    pub fn inline_identities(&self) -> &[InlineIdentity] {
        &self.inline_identities
    }

    pub fn with_inline_identity(
        mut self,
        label: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        self.inline_identities.push(InlineIdentity {
            label: label.into(),
            secret: secret.into(),
        });
        self
    }

    pub fn with_known_hosts(
        mut self,
        known_hosts_path: impl Into<PathBuf>,
        known_hosts_host: impl Into<String>,
        accept_new_host_keys: bool,
    ) -> Self {
        self.known_hosts_path = Some(known_hosts_path.into());
        self.known_hosts_host = Some(known_hosts_host.into());
        self.accept_new_host_keys = accept_new_host_keys;
        self
    }

    pub fn known_hosts_host(&self) -> &str {
        self.known_hosts_host
            .as_deref()
            .unwrap_or(self.host.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedEndpointChain {
    hops: Vec<ResolvedEndpoint>,
}

impl ResolvedEndpointChain {
    pub fn from_hops(hops: Vec<ResolvedEndpoint>) -> Self {
        Self { hops }
    }

    pub fn hops(&self) -> &[ResolvedEndpoint] {
        &self.hops
    }

    pub fn push(&mut self, endpoint: ResolvedEndpoint) {
        self.hops.push(endpoint);
    }
}

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("failed to read ssh config: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to resolve {0}: circular ProxyJump detected")]
    CircularJump(String),
}

#[derive(Debug, Clone)]
pub struct SshConfigResolver {
    sections: Vec<ConfigSection>,
}

impl SshConfigResolver {
    pub fn from_default_path() -> Result<Self, ResolveError> {
        let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
        Self::from_path(PathBuf::from(home).join(".ssh/config"))
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ResolveError> {
        let content = fs::read_to_string(path)?;
        Ok(Self {
            sections: parse_config(&content),
        })
    }

    pub fn resolve(&self, destination: &str) -> Result<ResolvedEndpointChain, ResolveError> {
        let mut visited = HashSet::new();
        let mut hops = Vec::new();
        self.resolve_into(destination, &mut visited, &mut hops)?;
        Ok(ResolvedEndpointChain { hops })
    }

    pub fn aliases(&self) -> Vec<String> {
        let mut aliases = HashSet::new();

        for section in &self.sections {
            for pattern in &section.patterns {
                let normalized = pattern.trim();
                if normalized.is_empty()
                    || normalized == "*"
                    || normalized.contains('*')
                    || normalized.contains('?')
                {
                    continue;
                }

                aliases.insert(normalized.to_string());
            }
        }

        let mut aliases = aliases.into_iter().collect::<Vec<_>>();
        aliases.sort();
        aliases
    }

    fn resolve_into(
        &self,
        destination: &str,
        visited: &mut HashSet<String>,
        hops: &mut Vec<ResolvedEndpoint>,
    ) -> Result<(), ResolveError> {
        let normalized = destination.trim().to_string();
        if !visited.insert(normalized.clone()) {
            return Err(ResolveError::CircularJump(normalized));
        }

        let entry = self.resolve_entry(&normalized);
        for jump in entry.proxy_jump_aliases() {
            self.resolve_into(jump, visited, hops)?;
        }

        let endpoint = entry.into_endpoint(&normalized);
        if hops.iter().all(|existing| existing.alias != endpoint.alias) {
            hops.push(endpoint);
        }

        visited.remove(&normalized);
        Ok(())
    }

    fn resolve_entry(&self, alias: &str) -> ResolvedConfigEntry {
        let mut entry = ResolvedConfigEntry::default();

        for section in &self.sections {
            if !section.matches(alias) {
                continue;
            }

            for (key, value) in &section.options {
                entry.apply_option(key, value);
            }
        }

        entry
    }
}

#[derive(Debug, Clone, Default)]
struct ResolvedConfigEntry {
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_files: Vec<PathBuf>,
    proxy_jump: Option<String>,
}

impl ResolvedConfigEntry {
    fn apply_option(&mut self, key: &str, value: &str) {
        match key {
            "hostname" if self.hostname.is_none() => self.hostname = Some(value.to_string()),
            "user" if self.user.is_none() => self.user = Some(value.to_string()),
            "port" if self.port.is_none() => self.port = value.parse::<u16>().ok(),
            "identityfile" => self.identity_files.push(expand_tilde(value)),
            "proxyjump" if self.proxy_jump.is_none() => self.proxy_jump = Some(value.to_string()),
            _ => {}
        }
    }

    fn proxy_jump_aliases(&self) -> Vec<&str> {
        self.proxy_jump
            .as_deref()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|alias| !alias.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn into_endpoint(self, alias: &str) -> ResolvedEndpoint {
        ResolvedEndpoint {
            alias: alias.to_string(),
            host: self.hostname.unwrap_or_else(|| alias.to_string()),
            user: self
                .user
                .unwrap_or_else(|| env::var("USER").unwrap_or_else(|_| String::from("root"))),
            port: self.port.unwrap_or(22),
            identity_files: self.identity_files,
            inline_identities: Vec::new(),
            known_hosts_path: None,
            known_hosts_host: None,
            accept_new_host_keys: false,
        }
    }
}

#[derive(Debug, Clone)]
struct ConfigSection {
    patterns: Vec<String>,
    options: Vec<(String, String)>,
}

impl ConfigSection {
    fn matches(&self, alias: &str) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern == "*" || pattern == alias)
    }
}

fn parse_config(content: &str) -> Vec<ConfigSection> {
    let mut sections = Vec::new();
    let mut current = ConfigSection {
        patterns: vec![String::from("*")],
        options: Vec::new(),
    };

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(keyword) = parts.next() else {
            continue;
        };
        let remainder = parts.collect::<Vec<_>>().join(" ");
        if remainder.is_empty() {
            continue;
        }

        if keyword.eq_ignore_ascii_case("host") {
            sections.push(current);
            current = ConfigSection {
                patterns: remainder
                    .split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
                options: Vec::new(),
            };
            continue;
        }

        current
            .options
            .push((keyword.to_ascii_lowercase(), remainder));
    }

    sections.push(current);
    sections
}

fn expand_tilde(value: &str) -> PathBuf {
    if let Some(stripped) = value.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }

    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::SshConfigResolver;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn resolves_proxy_jump_chain() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("univers-infra-ssh-test-{suffix}.config"));
        fs::write(
            &path,
            r#"
Host app-dev
  HostName app.example.com
  User davidxu

Host mechanism-dev
  HostName 10.0.0.2
  User david
  ProxyJump app-dev

Host automation-dev
  HostName 10.0.1.5
  User ubuntu
  ProxyJump mechanism-dev
"#,
        )
        .unwrap();

        let resolver = SshConfigResolver::from_path(&path).unwrap();
        let chain = resolver.resolve("automation-dev").unwrap();
        let aliases = chain
            .hops()
            .iter()
            .map(|endpoint| endpoint.alias.as_str())
            .collect::<Vec<_>>();

        assert_eq!(aliases, vec!["app-dev", "mechanism-dev", "automation-dev"]);
        let _ = fs::remove_file(path);
    }
}
