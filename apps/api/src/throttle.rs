//! Rate-limit policy for the endpoints a stranger can hammer.

use axum::extract::Request;
use axum::http::Method;
use axum::response::IntoResponse;

use crate::rate_limit;

struct Throttle {
    bucket: &'static str,
    limit: u32,
    window_secs: u64,
}

/// The routes a stranger can hammer without an account: passwords, discount
/// codes and the waiting list. Everything else already needs a session, so its
/// volume is bounded by the account that spends it.
fn throttle_for(path: &str) -> Option<Throttle> {
    let (bucket, limit, window_secs) = match path {
        "/api/auth/signup" => ("signup", 10, rate_limit::HOUR),
        "/api/auth/signin" => ("signin", 30, rate_limit::MINUTE),
        "/api/auth/forgot-password" => ("forgot", 10, rate_limit::HOUR),
        "/api/auth/wallet/challenge" => ("challenge", 30, rate_limit::MINUTE),
        // Low on purpose: a person applies a code or two, while guessing one
        // is the only way to find another seller's discount.
        "/api/discounts/validate" => ("discount", 12, rate_limit::MINUTE),
        // Recording a use follows a payment, so it is bounded by real invoices
        // rather than by guessing, and it is cheap to answer either way.
        "/api/discounts/use" => ("discount-use", 30, rate_limit::MINUTE),
        "/api/waitlist" => ("waitlist", 10, rate_limit::HOUR),
        // The relayer signs a settle for anyone who names an invoice, before any
        // wallet has signed, so the only protection in front of that signature
        // is a caller's pace and the deployment's daily cap. This bounds each
        // address, whatever wallet it names.
        "/api/sponsor/prepare" => ("sponsor-prepare", 30, rate_limit::MINUTE),
        "/api/sponsor/settle" => ("sponsor-settle", 30, rate_limit::MINUTE),
        // One claim per account is the handler's rule; this is the floor under
        // a script that retries, and it keeps the treasury from being probed.
        "/api/faucet" => ("faucet", 6, rate_limit::HOUR),
        // A subscription is a row per device, and a test send spends a push
        // service's quota: both are cheap, but neither should be a loop.
        "/api/push/subscribe" | "/api/push/unsubscribe" => ("push-device", 30, rate_limit::HOUR),
        "/api/push/test" => ("push-test", 12, rate_limit::HOUR),
        _ => return None,
    };
    Some(Throttle {
        bucket,
        limit,
        window_secs,
    })
}

/// Counts a hit per caller before the request reaches a handler. Only POST is
/// counted: a page read costs nothing, a guess at a password does.
pub(super) async fn throttle(
    request: Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if request.method() == Method::POST {
        if let Some(policy) = throttle_for(request.uri().path()) {
            let caller = rate_limit::caller(request.headers());
            if let Err(error) =
                rate_limit::hit(policy.bucket, &caller, policy.limit, policy.window_secs)
            {
                return error.into_response();
            }
        }
    }
    next.run(request).await
}
