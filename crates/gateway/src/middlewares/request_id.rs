use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::HeaderValue,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

/// Request ID header name
pub const REQUEST_ID_HEADER: &'static str = "X-Request-Id";

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
            service: Rc::new(service),
        }))
    }
}

pub struct RequestIdMiddleware<S> {
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
        // Check if request already has an ID (from upstream proxy)
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Store request ID in extensions for handlers to access
        req.extensions_mut()
            .insert(RequestIdExt(request_id.clone()));

        let service = self.service.clone();

        Box::pin(async move {
            let mut res = service.call(req).await?;

            // Add request ID to response headers
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-request-id"),
                HeaderValue::from_str(&request_id).unwrap(),
            );

            Ok(res)
        })
    }
}
