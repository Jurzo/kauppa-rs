//! Error pages (MB-0056, RR-0038). Not implemented yet: answers pass through.

use actix_web::Error;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;

#[cfg(test)]
mod tests;

/// The middleware around the RR-0010 scope.
pub async fn pages(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    next.call(req).await
}
