use axum::extract::{MatchedPath, Request};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use storyos_application::DiagnosticField;

/// Records one span for each request with its route template and status, never its path or body.
#[tracing::instrument(skip_all, fields(
    method = HttpMethod(request.method()).diagnostic(),
    route = RouteTemplate(request.extensions().get::<MatchedPath>()).diagnostic(),
    status = tracing::field::Empty,
))]
pub(crate) async fn request_span(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    tracing::Span::current().record("status", response.status().as_u16().diagnostic());
    response
}

struct HttpMethod<'a>(&'a Method);

impl DiagnosticField for HttpMethod<'_> {
    type Value<'b>
        = &'static str
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        match *self.0 {
            Method::GET => "GET",
            Method::HEAD => "HEAD",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::PATCH => "PATCH",
            Method::DELETE => "DELETE",
            Method::OPTIONS => "OPTIONS",
            _ => "other",
        }
    }
}

/// The template of a matched route, which the router defines, or `unmatched`.
struct RouteTemplate<'a>(Option<&'a MatchedPath>);

impl DiagnosticField for RouteTemplate<'_> {
    type Value<'b>
        = &'b str
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        self.0.map_or("unmatched", MatchedPath::as_str)
    }
}
