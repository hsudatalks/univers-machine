#[cfg(desktop)]
mod repository;
#[cfg(desktop)]
mod service;
#[cfg(desktop)]
mod store;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
pub(crate) use self::service::SecretManagementState;
#[cfg(mobile)]
pub(crate) use self::mobile::SecretManagementState;
