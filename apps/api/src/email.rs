//! Best-effort transactional email.
//!
//! Same shape as the Django setup Beacon uses: with no SMTP configured
//! (local development) the full message is printed to the console — the
//! equivalent of Django's `console.EmailBackend` — and with `EMAIL_HOST`
//! set it is delivered over SMTP. The expected free production setup is
//! Gmail SMTP with an app password (`smtp.gmail.com`, port 587, STARTTLS).
//!
//! Sends are blocking (SMTP is blocking); async callers should wrap
//! [`send`] in `spawn_blocking`. Delivery failures are logged, never
//! surfaced to the end user.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

/// The sender address: `DEFAULT_FROM_EMAIL`, falling back to the SMTP
/// username, then to a placeholder so a missing setting never blocks a send.
fn from_address() -> String {
    std::env::var("DEFAULT_FROM_EMAIL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            std::env::var("EMAIL_HOST_USER")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "no-reply@payvand.local".to_string())
        })
}

/// Sends one plain-text email.
///
/// With `EMAIL_HOST` unset the message — link included — is printed to the
/// console instead, so the reset flow works end to end with zero setup.
pub fn send(to: &str, subject: &str, body: &str) -> Result<(), String> {
    let from = from_address();

    let host = std::env::var("EMAIL_HOST").unwrap_or_default();
    let host = host.trim().to_string();
    if host.is_empty() {
        eprintln!("[email] EMAIL_HOST not set — printing instead of sending");
        eprintln!("To: {to}");
        eprintln!("From: {from}");
        eprintln!("Subject: {subject}");
        eprintln!("---8<---");
        eprintln!("{body}");
        eprintln!("--->8---");
        return Ok(());
    }

    let port: u16 = std::env::var("EMAIL_PORT")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(587);
    let username = std::env::var("EMAIL_HOST_USER").unwrap_or_default();
    let password = std::env::var("EMAIL_HOST_PASSWORD").unwrap_or_default();
    // STARTTLS on; `EMAIL_USE_TLS=false` is the escape hatch for servers
    // that genuinely cannot do it.
    let use_tls = std::env::var("EMAIL_USE_TLS")
        .map(|value| value.trim() != "false")
        .unwrap_or(true);

    let message = Message::builder()
        .from(
            from.parse()
                .map_err(|error| format!("bad DEFAULT_FROM_EMAIL: {error}"))?,
        )
        .to(to
            .parse()
            .map_err(|error| format!("bad recipient address: {error}"))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|error| format!("could not build the message: {error}"))?;

    let mut builder = if use_tls {
        SmtpTransport::relay(&host).map_err(|error| format!("SMTP setup failed: {error}"))?
    } else {
        SmtpTransport::builder_dangerous(&host)
    };
    builder = builder.port(port);
    if !username.is_empty() {
        builder = builder.credentials(Credentials::new(username, password));
    }

    builder
        .build()
        .send(&message)
        .map_err(|error| format!("SMTP send failed: {error}"))?;
    Ok(())
}
