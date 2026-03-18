use anyhow::Context;
use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Start a standalone HTTP server that exposes `GET /metrics` in Prometheus
/// text exposition format.
///
/// The caller must pass the [`PrometheusHandle`] obtained from
/// [`install_prometheus()`](crate::install_prometheus). This keeps the HTTP
/// exporter decoupled from the recorder installation.
pub async fn serve_metrics_http(host: &str, port: u16, handle: PrometheusHandle) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse().context("parse metrics bind address")?;

    let listener = TcpListener::bind(addr).await.context("bind metrics tcp listener")?;

    tracing::info!("metrics server listening on {addr}");

    loop {
        let (stream, _) = listener.accept().await.context("accept tcp connection")?;
        let handle = handle.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let svc = service_fn(move |req: Request<hyper::body::Incoming>| {
                let handle = handle.clone();
                async move { Ok::<_, hyper::Error>(metrics_response(req, &handle)) }
            });

            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!(error = %err, "metrics connection error");
            }
        });
    }
}

fn metrics_response(req: Request<hyper::body::Incoming>, handle: &PrometheusHandle) -> Response<Full<Bytes>> {
    if req.method() == Method::GET && req.uri().path() == "/metrics" {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4")
            .body(Full::new(Bytes::from(handle.render())))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("internal error"))))
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("not found")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("not found"))))
    }
}
