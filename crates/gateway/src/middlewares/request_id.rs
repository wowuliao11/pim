use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

/// Request ID header name
pub const REQUEST_ID_HEADER: &str = "X-Request-Id";

/// Request ID key for extracting from request extensions
#[derive(Debug, Clone)]
pub struct RequestIdExt(pub String);

/// Middleware to add a unique request ID to each request
/// The ID is added to response headers and available in request extensions
pub struct RequestId;

impl<S, B> Transform<S, ServiceRequest> for RequestId
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestIdMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdMiddleware {
            header_name: HeaderName::from_static("x-request-id"),
            service: Rc::new(service),
        }))
    }
}

pub struct RequestIdMiddleware<S> {
    header_name: HeaderName,
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddleware<S>
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
        // Cache the HeaderName once per middleware instance (not per request)
        let header_name = self.header_name.clone();

        // Check if request already has an ID (from upstream proxy) and ensure it's safe
        // to use as an HTTP header value.
        let (request_id, request_id_value) = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| HeaderValue::from_str(s).ok().map(|hv| (s.to_string(), hv)))
            .unwrap_or_else(|| {
                let id = Uuid::new_v4().to_string();
                // UUID should always be valid as a header value; if something goes wrong,
                // fall back to an empty header value.
                let hv = HeaderValue::from_str(&id).unwrap_or_else(|_| HeaderValue::from_static(""));
                (id, hv)
            });

        // Store request ID in extensions for handlers to access
        req.extensions_mut().insert(RequestIdExt(request_id.clone()));

        let service = self.service.clone();

        Box::pin(async move {
            let mut res = service.call(req).await?;

            // Add request ID to response headers
            res.headers_mut().insert(header_name, request_id_value);

            Ok(res)
        })
    }
}
