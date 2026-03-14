use std::path::PathBuf;

pub(crate) fn univers_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| String::from("Failed to resolve user home directory"))?;

    Ok(home.join(".univers"))
}

pub(crate) fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
