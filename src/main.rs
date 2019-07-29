#![feature(async_await)]
#![warn(clippy::all)]

use std::{sync::Arc, time::Instant};

use futures::lock::Mutex;
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Response, Server};

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
    shutdown_tx: Mutex<Option<futures::channel::oneshot::Sender<()>>>,
    shutdown_at: Mutex<Option<Instant>>,
}

#[tokio::main(single_thread)]
async fn main() {
    env_logger::init();

    let port = std::env::var("PORT")
        .map(|s| s.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);

    let (tx, rx) = futures::channel::oneshot::channel::<()>();

    let app = Arc::new(App {
        client: Client::new(),
        port,
        shutdown_tx: Mutex::new(Some(tx)),
        shutdown_at: Mutex::new(None),
    });

    let make_service = make_service_fn(|_| {
        let app = app.clone();
        async { Ok::<_, Error>(service_fn(move |req| middleware(app.clone(), req))) }
    });

    let server = Server::bind(&([0, 0, 0, 0], port).into())
        .serve(make_service)
        .with_graceful_shutdown(async {
            rx.await.ok();
        });

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    let shutdown_duration = Instant::now().duration_since(app.shutdown_at.lock().await.unwrap());
    println!("Server gracefuly shutdown, taking {:?}", shutdown_duration);
}
