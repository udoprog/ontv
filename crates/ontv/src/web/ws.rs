use std::borrow::Cow;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::ConnectInfo;
use axum::response::IntoResponse;
use axum::Extension;
use musli::mode::Binary;
use musli::reader::SliceReader;
use musli::Encode;
use musli_axum::{api, ws};
use rand::prelude::*;
use rand::rngs::SmallRng;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tokio_stream::StreamExt;
use tracing::{Instrument, Level};

use crate::service::Service;

struct Handler {
    service: Arc<RwLock<Service>>,
}

impl Handler {
    fn new(service: Arc<RwLock<Service>>) -> Self {
        Self { service }
    }
}

impl ws::Handler for Handler {
    type Error = anyhow::Error;

    fn handle<'this>(
        &'this mut self,
        incoming: &'this mut ws::Incoming<'_>,
        outgoing: &'this mut ws::Outgoing<'_>,
        kind: &'this str,
    ) -> impl Future<Output = Result<()>> {
        async move {
            match kind {
                "initialize-dashboard" => {
                    let service = self.service.read().await;
                    outgoing.write(super::dashboard_update(&service));
                }
                _ => {}
            }

            Ok(())
        }
    }
}

pub(super) async fn entry(
    ws: WebSocketUpgrade,
    Extension(service): Extension<Arc<RwLock<Service>>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let span = tracing::span!(Level::INFO, "websocket", ?remote);

        let mut server = ws::Server::new(socket, Handler::new(service));

        if let Err(error) = server.run().instrument(span).await {
            tracing::error!(?error);
        }
    })
}
