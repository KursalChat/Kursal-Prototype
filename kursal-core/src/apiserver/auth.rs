use crate::apiserver::APIAppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::{net::SocketAddr, time::Instant};

pub async fn auth_middleware(
    State(state): State<APIAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip();
    let now = Instant::now();

    {
        let mut map = state.rate_map.lock().await;
        if map.entry(ip).or_default().is_limited(now) {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    let token_match = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|v| {
            v.len() == state.auth_token.len()
                && subtle::ConstantTimeEq::ct_eq(v.as_bytes(), state.auth_token.as_bytes()).into()
        });

    if token_match {
        state.rate_map.lock().await.entry(ip).or_default().reset();
        Ok(next.run(req).await)
    } else {
        state
            .rate_map
            .lock()
            .await
            .entry(ip)
            .or_default()
            .record_failure(now);

        Err(StatusCode::UNAUTHORIZED)
    }
}
