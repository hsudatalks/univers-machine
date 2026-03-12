pub(crate) fn service_key(target_id: &str, service_id: &str) -> String {
    format!("{}::{}", target_id, service_id)
}

pub(crate) fn surface_key(target_id: &str, surface_id: &str) -> String {
    service_key(target_id, surface_id)
}
