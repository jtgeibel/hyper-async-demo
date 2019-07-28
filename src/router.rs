use crate::endpoints::*;
use crate::{App, AppResponse};
use std::sync::Arc;

use hyper::{Body, Request, Response};

pub(super) async fn router(app: Arc<App>, req: Request<Body>) -> AppResponse {
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
