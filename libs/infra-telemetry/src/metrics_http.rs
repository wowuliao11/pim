use crate as telemetry;
use anyhow::Context;
use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn serve_metrics_http(host: &str, port: u16) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse().context("parse metrics bind address")?;

    let listener = TcpListener::bind(addr).await.context("bind metrics tcp listener")?;

    loop {
        let (stream, _) = listener.accept().await.context("accept tcp connection")?;

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let svc =
                service_fn(|req: Request<hyper::body::Incoming>| async move { Ok::<_, hyper::Error>(handle(req)) });

            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!(error = %err, "metrics connection error");
            }
        });
    }
}

fn handle(req: Request<hyper::body::Incoming>) -> Response<Full<Bytes>> {
    if req.method() == Method::GET && req.uri().path() == "/metrics" {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4")
            .body(Full::new(Bytes::from(telemetry::render())))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("internal error"))))
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("not found")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("not found"))))
    }
}
