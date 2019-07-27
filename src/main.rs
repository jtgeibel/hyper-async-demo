#![feature(async_await)]

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::{join, prelude::*};
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Error, Request, Response, Server, Uri};

const DEFAULT_PORT: u16 = 3000;

struct App {
    client: Client<HttpConnector>,
    port: u16,
    shutdown_tx: Mutex<Option<futures::channel::oneshot::Sender<()>>>,
    shutdown_at: Mutex<Option<Instant>>,
}

#[tokio::main(single_thread)]
async fn main() {
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
        async { Ok::<_, Error>(service_fn(move |req| router(app.clone(), req))) }
    });

    let server = Server::bind(&([127, 0, 0, 1], port).into())
        .serve(make_service)
        .with_graceful_shutdown(async {
            rx.await.ok();
        });

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    let shutdown_duration = Instant::now().duration_since(app.shutdown_at.lock().unwrap().unwrap());
    println!("Server gracefuly shutdown, taking {:?}", shutdown_duration);
}

async fn router(app: Arc<App>, req: Request<Body>) -> Result<Response<Body>, Error> {
    match req.uri().path() {
        "/" => Ok(index()),
        "/multi" => multi(&app).await,
        "/port" => Ok(port(&app)),
        "/panic" => panic!("Intentional panic from `/panic`"), // FIXME: takes down the whole server. because of single threaded runtime, file upstream bug?
        "/pause" => Ok(pause(req).await),
        "/shutdown" => Ok(shutdown(&app).await),
        _ => Ok(not_found()),
    }
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(404)
        .body("Not found".into())
        .unwrap()
}

fn index() -> Response<Body> {
    Response::builder()
        .status(200)
        .body("Hello from `/`".into())
        .unwrap()
}

fn port(app: &Arc<App>) -> Response<Body> {
    Response::builder()
        .status(200)
        .body(app.port.to_string().into())
        .unwrap()
}

async fn multi(app: &Arc<App>) -> Result<Response<Body>, Error> {
    let authority = format!("127.0.0.1:{}", app.port);
    let build_uri = |p_and_q| {
        Uri::builder()
            .scheme("http")
            .authority(authority.as_str())
            .path_and_query(p_and_q)
            .build()
            .unwrap()
    };

    let begin_at = Instant::now();

    // Spawn concurrent requests to slow endpoints
    let fut1 = app.client.get(build_uri("/pause?1000"));
    let fut2 = app.client.get(build_uri("/pause?5000"));
    let (res1, res2) = join!(fut1, fut2);

    let duration = Instant::now().duration_since(begin_at);
    let message = format!(
        "Total duration: {:?}, Response 1: {}, Response 2: {}",
        duration,
        String::from_utf8_lossy(&res1.unwrap().into_body().try_concat().await.unwrap()),
        String::from_utf8_lossy(&res2.unwrap().into_body().try_concat().await.unwrap()),
    );
    Ok(Response::builder()
        .status(200)
        .body(message.into())
        .unwrap())
}

async fn shutdown(app: &Arc<App>) -> Response<Body> {
    let message = match app.shutdown_tx.lock().unwrap().take() {
        Some(tx) => {
            *app.shutdown_at.lock().unwrap() = Some(Instant::now());
            tx.send(()).ok();
            "Initiating graceful shutdown"
        }
        None => "Graceful shutdown already in progress",
    };

    Response::builder()
        .status(200)
        .body(message.into())
        .unwrap()
}

async fn pause(req: Request<Body>) -> Response<Body> {
    const DEFAULT_DELAY: u64 = 500;
    let millis = req
        .uri()
        .query()
        .map(|q| q.parse().unwrap_or(DEFAULT_DELAY))
        .unwrap_or(DEFAULT_DELAY);
    tokio::timer::Delay::new(Instant::now() + Duration::from_millis(millis)).await;
    let message = format!("Paused for {} ms.", millis);
    Response::builder()
        .status(200)
        .body(message.into())
        .unwrap()
}
