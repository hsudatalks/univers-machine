use std::{env, fs, path::PathBuf, sync::Arc};

use russh::{client, client::Handle, keys::PrivateKeyWithHashAlg, Preferred};

use crate::{
    ssh_config::{ResolvedEndpoint, ResolvedEndpointChain},
    types::{ClientOptions, RusshError},
};

pub(crate) struct ClientConnection {
    pub(crate) handle: Handle<ClientHandler>,
}

pub(crate) async fn connect_chain(
    chain: &ResolvedEndpointChain,
    options: &ClientOptions,
) -> Result<ClientConnection, RusshError> {
    let mut current_handle: Option<Handle<ClientHandler>> = None;

    for endpoint in chain.hops() {
        let next = if let Some(handle) = current_handle.take() {
            connect_via_handle(handle, endpoint, options).await?
        } else {
            connect_endpoint(endpoint, options).await?
        };

        current_handle = Some(next);
    }

    Ok(ClientConnection {
        handle: current_handle.ok_or_else(|| {
            RusshError::ResolveDestination(String::from("resolved ssh chain was empty"))
        })?,
    })
}

async fn connect_endpoint(
    endpoint: &ResolvedEndpoint,
    options: &ClientOptions,
) -> Result<Handle<ClientHandler>, RusshError> {
    let config = client_config(options);
    let mut handle = client::connect(
        config,
        (endpoint.host.as_str(), endpoint.port),
        ClientHandler::new(endpoint),
    )
    .await?;
    authenticate_endpoint(&mut handle, endpoint).await?;
    Ok(handle)
}

async fn connect_via_handle(
    handle: Handle<ClientHandler>,
    endpoint: &ResolvedEndpoint,
    options: &ClientOptions,
) -> Result<Handle<ClientHandler>, RusshError> {
    let channel = handle
        .channel_open_direct_tcpip(
            endpoint.host.clone(),
            endpoint.port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;
    let stream = channel.into_stream();
    let config = client_config(options);
    let mut nested = client::connect_stream(config, stream, ClientHandler::new(endpoint)).await?;
    authenticate_endpoint(&mut nested, endpoint).await?;
    Ok(nested)
}

async fn authenticate_endpoint(
    handle: &mut Handle<ClientHandler>,
    endpoint: &ResolvedEndpoint,
) -> Result<(), RusshError> {
    let mut candidates = endpoint.identity_files().to_vec();
    if candidates.is_empty() {
        candidates.extend(default_identity_files());
    }

    for path in candidates {
        if fs::metadata(&path).is_err() {
            continue;
        }

        let key = russh::keys::load_secret_key(&path, None)?;
        let auth = handle
            .authenticate_publickey(
                endpoint.user.clone(),
                PrivateKeyWithHashAlg::new(
                    Arc::new(key),
                    handle.best_supported_rsa_hash().await?.flatten(),
                ),
            )
            .await?;

        if auth.success() {
            return Ok(());
        }
    }

    if endpoint.identity_files().is_empty() && default_identity_files().is_empty() {
        return Err(RusshError::MissingIdentity(endpoint.alias.clone()));
    }

    Err(RusshError::Auth(
        endpoint.user.clone(),
        endpoint.host.clone(),
    ))
}

fn client_config(options: &ClientOptions) -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: options.inactivity_timeout,
        keepalive_interval: options.keepalive_interval,
        keepalive_max: options.keepalive_max,
        preferred: Preferred {
            kex: std::borrow::Cow::Owned(vec![
                russh::kex::CURVE25519_PRE_RFC_8731,
                russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
            ]),
            ..Default::default()
        },
        nodelay: true,
        ..Default::default()
    })
}

#[derive(Clone)]
pub(crate) struct ClientHandler {
    endpoint: ResolvedEndpoint,
}

impl ClientHandler {
    fn new(endpoint: &ResolvedEndpoint) -> Self {
        Self {
            endpoint: endpoint.clone(),
        }
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let Some(known_hosts_path) = self.endpoint.known_hosts_path.as_ref() else {
            return Ok(true);
        };

        let known_hosts_host = self.endpoint.known_hosts_host();
        match russh::keys::check_known_hosts_path(
            known_hosts_host,
            self.endpoint.port,
            server_public_key,
            known_hosts_path,
        ) {
            Ok(true) => Ok(true),
            Ok(false) if self.endpoint.accept_new_host_keys => {
                russh::keys::known_hosts::learn_known_hosts_path(
                    known_hosts_host,
                    self.endpoint.port,
                    server_public_key,
                    known_hosts_path,
                )
                .map_err(russh::Error::from)?;
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(error) => Err(russh::Error::from(error)),
        }
    }
}

fn default_identity_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let home = env::var("HOME").ok();
    let Some(home) = home else {
        return paths;
    };

    for name in ["id_ed25519", "id_rsa"] {
        let path = PathBuf::from(&home).join(".ssh").join(name);
        if fs::metadata(&path).is_ok() {
            paths.push(path);
        }
    }

    paths
}
