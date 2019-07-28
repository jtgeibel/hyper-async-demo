use crate::{App, AppResponse};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{join, prelude::*};
use hyper::{Body, Request, Response, Uri};
use tokio::timer::Delay;

pub(super) fn index() -> AppResponse {
    Response::builder()
        .status(200)
        .body("Hello from `/`".into())
        .map_err(Into::into)
}

pub(super) fn port(app: &Arc<App>) -> AppResponse {
    Response::builder()
        .status(200)
        .body(app.port.to_string().into())
        .map_err(Into::into)
}

pub(super) async fn multi(app: &Arc<App>) -> AppResponse {
    let authority = format!("127.0.0.1:{}", app.port);
    let build_uri = |p_and_q| {
        Uri::builder()
            .scheme("http")
            .authority(authority.as_str())
            .path_and_query(p_and_q)
            .build()
    };

    let begin_at = Instant::now();

    // Spawn concurrent requests to slow endpoints
    let fut1 = app.client.get(build_uri("/pause?1000")?);
    let fut2 = app.client.get(build_uri("/pause?5000")?);
    let (res1, res2) = join!(fut1, fut2);

    let duration = Instant::now().duration_since(begin_at);
    let message = format!(
        "Total duration: {:?}, Response 1: {}, Response 2: {}",
        duration,
        String::from_utf8_lossy(&res1?.into_body().try_concat().await?),
        String::from_utf8_lossy(&res2?.into_body().try_concat().await?),
    );
    Response::builder()
        .status(200)
        .body(message.into())
        .map_err(Into::into)
}

pub(super) async fn shutdown(app: &Arc<App>) -> AppResponse {
    let message = match app.shutdown_tx.lock().await.take() {
        Some(tx) => {
            *app.shutdown_at.lock().await = Some(Instant::now());
            tx.send(()).ok();
            "Initiating graceful shutdown"
        }
        None => "Graceful shutdown already in progress",
    };

    Response::builder()
        .status(200)
        .body(message.into())
        .map_err(Into::into)
}

pub(super) async fn pause(req: Request<Body>) -> AppResponse {
    const DEFAULT_DELAY: u64 = 500;
    let millis = req
        .uri()
        .query()
        .map(|q| q.parse().unwrap_or(DEFAULT_DELAY))
        .unwrap_or(DEFAULT_DELAY);
    Delay::new(Instant::now() + Duration::from_millis(millis)).await;
    let message = format!("Paused for {} ms.", millis);
    Response::builder()
        .status(200)
        .body(message.into())
        .map_err(Into::into)
}
