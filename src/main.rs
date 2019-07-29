#![feature(async_await)]
#![warn(clippy::all)]

use std::{sync::Arc, time::Instant};

use futures::{channel::oneshot, lock::Mutex};
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Response, Server};
use log::info;

mod endpoints;
mod error;
mod middleware;
mod router;

use error::Error;
use middleware::middleware;

const DEFAULT_PORT: u16 = 3000;

type AppResponse = Result<Response<Body>, Error>;

struct App {
    client: Client<HttpConnector>,
    port: u16,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    shutdown_at: Mutex<Option<Instant>>,
}

#[tokio::main(single_thread)]
async fn main() {
    env_logger::init();

    let port = std::env::var("PORT")
        .map(|s| s.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);

    // Set up signal handler on SIGTERM and SIGINT for graceful shutdown
    let (signal_tx, signal_rx) = oneshot::channel::<()>();
    let signal_tx = std::sync::Mutex::new(Some(signal_tx));
    let signal_shutdown_at = Arc::new(std::sync::Mutex::new(None));
    let signal_shutdown_at2 = signal_shutdown_at.clone();
    ctrlc::set_handler(move || {
        info!("Starting a graceful shutdown from the signal handler");
        *signal_shutdown_at2.lock().unwrap() = Some(Instant::now());
        signal_tx
            .lock()
            .map(|mut opt| opt.take().map(|tx| tx.send(())))
            .ok();
    })
    .unwrap();

    let (app_tx, app_rx) = oneshot::channel::<()>();

    let app = Arc::new(App {
        client: Client::new(),
        port,
        shutdown_tx: Mutex::new(Some(app_tx)),
        shutdown_at: Mutex::new(None),
    });

    let make_service = make_service_fn(|_| {
        let app = app.clone();
        async { Ok::<_, Error>(service_fn(move |req| middleware(app.clone(), req))) }
    });

    let server = Server::bind(&([0, 0, 0, 0], port).into())
        .serve(make_service)
        .with_graceful_shutdown(async {
            futures::future::select(signal_rx, app_rx).await;
        });

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    let shutdown_at = app
        .shutdown_at
        .lock()
        .await
        .unwrap_or_else(|| signal_shutdown_at.lock().unwrap().unwrap());
    let shutdown_duration = Instant::now().duration_since(shutdown_at);
    println!("Server gracefuly shutdown, taking {:?}", shutdown_duration);
}
