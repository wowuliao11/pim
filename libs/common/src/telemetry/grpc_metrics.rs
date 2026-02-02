use crate::telemetry::{
    ERROR_KIND_LOGIC, ERROR_KIND_SYSTEM, LABEL_ERROR_KIND, LABEL_METHOD, LABEL_STATUS_CODE,
    METRIC_RPC_DURATION_SECONDS, METRIC_RPC_ERRORS_TOTAL, METRIC_RPC_REQUESTS_TOTAL,
};
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};

#[derive(Clone, Default)]
pub struct GrpcMetricsLayer;

impl<S> Layer<S> for GrpcMetricsLayer {
    type Service = GrpcMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcMetricsService { inner }
    }
}

#[derive(Clone)]
pub struct GrpcMetricsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for GrpcMetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: 'static,
    ResBody: Body<Data = Bytes> + 'static,
    ResBody::Error: 'static,
{
    type Response = http::Response<MetricsBody<ResBody>>;
    type Error = S::Error;
    type Future = futures_util::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let method = req.uri().path().to_string();
        let started = Instant::now();

        let fut = self.inner.call(req);

        Box::pin(async move {
            let ctx = Arc::new(CallCtx {
                method,
                started,
                recorded: AtomicBool::new(false),
            });

            match fut.await {
                Ok(resp) => {
                    let (parts, body) = resp.into_parts();
                    let body = MetricsBody { inner: body, ctx };
                    Ok(http::Response::from_parts(parts, body))
                }
                Err(err) => {
                    ctx.record("transport_error");
                    Err(err)
                }
            }
        })
    }
}

struct CallCtx {
    method: String,
    started: Instant,
    recorded: AtomicBool,
}

impl CallCtx {
    fn record(&self, grpc_status: &str) {
        if self
            .recorded
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let duration = self.started.elapsed().as_secs_f64();

        metrics::counter!(
            METRIC_RPC_REQUESTS_TOTAL,
            LABEL_METHOD => self.method.clone(),
            LABEL_STATUS_CODE => grpc_status.to_string()
        )
        .increment(1);

        metrics::histogram!(METRIC_RPC_DURATION_SECONDS, LABEL_METHOD => self.method.clone()).record(duration);

        if grpc_status != "0" {
            let error_kind = classify_grpc_error_kind(grpc_status);
            metrics::counter!(
                METRIC_RPC_ERRORS_TOTAL,
                LABEL_METHOD => self.method.clone(),
                LABEL_ERROR_KIND => error_kind
            )
            .increment(1);
        }
    }
}

pin_project! {
    pub struct MetricsBody<B> {
        #[pin]
        inner: B,
        ctx: Arc<CallCtx>,
    }
}

impl<B> Body for MetricsBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        match this.inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => match frame.into_trailers() {
                Ok(trailers) => {
                    let status = trailers
                        .get("grpc-status")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("unknown");
                    this.ctx.record(status);
                    Poll::Ready(Some(Ok(Frame::trailers(trailers))))
                }
                Err(frame) => Poll::Ready(Some(Ok(frame))),
            },
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

fn classify_grpc_error_kind(grpc_status: &str) -> String {
    // gRPC canonical codes are numeric strings (0..=16).
    // We classify common client/validation failures as `logic`, and infrastructure/timeouts as `system`.
    match grpc_status {
        // INVALID_ARGUMENT, NOT_FOUND, ALREADY_EXISTS, PERMISSION_DENIED, UNAUTHENTICATED,
        // FAILED_PRECONDITION, OUT_OF_RANGE
        "3" | "5" | "6" | "7" | "16" | "9" | "11" => ERROR_KIND_LOGIC.to_string(),

        // DEADLINE_EXCEEDED, RESOURCE_EXHAUSTED, ABORTED, INTERNAL, UNAVAILABLE, DATA_LOSS, UNKNOWN
        "4" | "8" | "10" | "13" | "14" | "15" | "2" => ERROR_KIND_SYSTEM.to_string(),

        _ => ERROR_KIND_SYSTEM.to_string(),
    }
}
