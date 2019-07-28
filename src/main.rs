#![feature(async_await)]
#![warn(clippy::all)]

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{join, lock::Mutex, prelude::*};
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Uri};
use log::{error, info};

mod error;
use error::Error;

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

    let server = Server::bind(&([127, 0, 0, 1], port).into())
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

async fn middleware(app: Arc<App>, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let begin_at = Instant::now();
    let path = req
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_else(Default::default);

    let response = std::panic::AssertUnwindSafe(router(app, req));
    let response = response
        .catch_unwind()
        .unwrap_or_else(|e| {
            let desc = match e.downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => match e.downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<Any>",
                },
            };
            Err(Error::ApplicationPanic(desc.to_string()))
        })
        .await
        .or_else(|e| {
            error!("Application error: {}", e);
            Ok(Response::builder()
                .status(500)
                .body("Internal server error".into())
                .unwrap())
        });

    info!(
        "Request to `{}` took {:?}",
        path,
        Instant::now().duration_since(begin_at)
    );
    response
}

async fn router(app: Arc<App>, req: Request<Body>) -> AppResponse {
    match req.uri().path() {
        "/" => index(),
        "/error" => Err(Default::default()),
        "/multi" => multi(&app).await,
        "/port" => port(&app),
        "/panic" => panic!("Intentional panic from `/panic`"),
        "/pause" => pause(req).await,
        "/shutdown" => shutdown(&app).await,
        _ => not_found(),
    }
}

fn not_found() -> AppResponse {
    Response::builder()
        .status(404)
        .body("Not found".into())
        .map_err(Into::into)
}

fn index() -> AppResponse {
    Response::builder()
        .status(200)
        .body("Hello from `/`".into())
        .map_err(Into::into)
}

fn port(app: &Arc<App>) -> AppResponse {
    Response::builder()
        .status(200)
        .body(app.port.to_string().into())
        .map_err(Into::into)
}

async fn multi(app: &Arc<App>) -> AppResponse {
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

async fn shutdown(app: &Arc<App>) -> AppResponse {
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

async fn pause(req: Request<Body>) -> AppResponse {
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
        .map_err(Into::into)
}
