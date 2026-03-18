use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use infra_telemetry::{
    ERROR_KIND_LOGIC, ERROR_KIND_SYSTEM, LABEL_ERROR_KIND, LABEL_METHOD, LABEL_STATUS_CODE,
    METRIC_RPC_DURATION_SECONDS, METRIC_RPC_ERRORS_TOTAL, METRIC_RPC_REQUESTS_TOTAL,
};
use std::rc::Rc;
use std::time::Instant;

pub struct HttpMetrics;

impl<S, B> Transform<S, ServiceRequest> for HttpMetrics
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = HttpMetricsMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HttpMetricsMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct HttpMetricsMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for HttpMetricsMiddleware<S>
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
        let started = Instant::now();

        // Prefer the route pattern to avoid high-cardinality labels.
        let pattern = req.match_pattern().map(|s| s.to_string());
        let method_label = match pattern {
            Some(p) => format!("{} {p}", req.method()),
            None => "unknown".to_string(),
        };

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            let status = res.status().as_u16();
            let duration = started.elapsed().as_secs_f64();

            metrics::counter!(
                METRIC_RPC_REQUESTS_TOTAL,
                LABEL_METHOD => method_label.clone(),
                LABEL_STATUS_CODE => status.to_string()
            )
            .increment(1);

            metrics::histogram!(METRIC_RPC_DURATION_SECONDS, LABEL_METHOD => method_label.clone()).record(duration);

            if status >= 400 {
                let kind = if status >= 500 {
                    ERROR_KIND_SYSTEM
                } else {
                    ERROR_KIND_LOGIC
                };

                metrics::counter!(
                    METRIC_RPC_ERRORS_TOTAL,
                    LABEL_METHOD => method_label,
                    LABEL_ERROR_KIND => kind
                )
                .increment(1);
            }

            Ok(res)
        })
    }
}

// Ensure the middleware is `Clone`-friendly at the app layer.
impl Clone for HttpMetrics {
    fn clone(&self) -> Self {
        Self
    }
}
