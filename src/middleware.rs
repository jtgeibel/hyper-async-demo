use crate::error::Error;
use crate::router::router;
use crate::App;
use std::any::Any;
use std::sync::Arc;
use std::panic::AssertUnwindSafe;
use std::time::Instant;

use futures::prelude::*;
use hyper::{Body, Request, Response};
use log::{error, info};

pub(super) async fn middleware(
    app: Arc<App>,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    let begin_at = Instant::now();
    let path = req
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_else(Default::default);

    let response = AssertUnwindSafe(router(app, req));
    let response = response
        .catch_unwind()
        .unwrap_or_else(panic_handler)
        .await
        .or_else(error_handler);

    info!(
        "Request to `{}` took {:?}",
        path,
        Instant::now().duration_since(begin_at)
    );
    response
}

fn panic_handler(err: Box<dyn Any + Send + 'static>) -> Result<Response<Body>, Error> {
    let desc = match err.downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match err.downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    };
    Err(Error::ApplicationPanic(desc.to_string()))
}

fn error_handler(err: Error) -> Result<Response<Body>, hyper::Error> {
    error!("Application error: {}", err);
    Ok(Response::builder()
        .status(500)
        .body("Internal server error".into())
        .unwrap())
}
