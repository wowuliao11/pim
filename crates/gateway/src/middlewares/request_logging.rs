use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Instant,
};

use actix_web::HttpMessage;
use actix_web::ResponseError;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures_util::future::LocalBoxFuture;
use tracing::Instrument;

use crate::errors::AppError;
use crate::middlewares::request_id::RequestIdExt;

/// Logs one structured event per request (plus an optional start event).
///
/// Required fields:
/// - request_id (if present)
/// - method
/// - path (no query string)
/// - status
/// - latency_ms
///
/// Forbidden fields:
/// - request/response bodies
/// - headers
pub struct RequestLogging;

impl<S, B> Transform<S, ServiceRequest> for RequestLogging
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestLoggingMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLoggingMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct RequestLoggingMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestLoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        let method = req.method().to_string();
        let path = req.path().to_string();
        let request_id = req.extensions().get::<RequestIdExt>().map(|v| v.0.clone());

        let started = Instant::now();

        let span = match &request_id {
            Some(id) => tracing::info_span!(
                "http_request",
                request_id = %id,
                method = %method,
                path = %path
            ),
            None => tracing::info_span!("http_request", method = %method, path = %path),
        };

        {
            let _guard = span.enter();
            tracing::info!("request_started");
        }

        Box::pin(
            async move {
                match service.call(req).await {
                    Ok(res) => {
                        let status = res.status().as_u16();
                        let latency_ms = started.elapsed().as_millis();

                        if status >= 500 {
                            tracing::error!(status = status, latency_ms = latency_ms, "request_finished");
                        } else if status >= 400 {
                            tracing::warn!(status = status, latency_ms = latency_ms, "request_finished");
                        } else {
                            tracing::info!(status = status, latency_ms = latency_ms, "request_finished");
                        }

                        Ok(res)
                    }
                    Err(err) => {
                        // Log exactly once at the request boundary.
                        let latency_ms = started.elapsed().as_millis() as u64;

                        if let Some(app_err) = err.as_error::<AppError>() {
                            let status = app_err.status_code().as_u16();
                            let error_kind = app_err.kind();

                            tracing::error!(
                                status = status,
                                latency_ms = latency_ms,
                                error_kind = %error_kind,
                                error = %app_err,
                                "request_failed"
                            );
                        } else {
                            tracing::error!(latency_ms = latency_ms, error = ?err, "request_failed");
                        }

                        Err(err)
                    }
                }
            }
            .instrument(span),
        )
    }
}
