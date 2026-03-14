use crate::agent::event::{HookEvent, SessionSnapshot};
use anyhow::Result;

pub trait SessionRepository: Send + Sync {
    fn list_sessions(&self, include_ended: bool) -> Result<Vec<SessionSnapshot>>;
    fn persist_event(&self, event: &HookEvent) -> Result<()>;
    fn clean_old(&self, hours: u32) -> Result<usize>;
}
