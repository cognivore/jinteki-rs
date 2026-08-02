//! Magic-link delivery: SendGrid HTTP API v3 when `SENDGRID_API_KEY` is
//! set, dev mode (log the link, send nothing) otherwise — exactly
//! draftroom's `email.go` contract (ACCOUNTS-AND-DECKS.md §4.3).
//!
//! Env: `SENDGRID_API_KEY` (secret), `FROM_EMAIL`, `FROM_NAME`, `APP_URL`.
//! The FROM address/domain is deliberately configuration, not code: mails
//! whose links point at a domain unrelated to the FROM domain get
//! spam-filtered (the draftroom incident, OI-1) — the operator picks an
//! aligned, warmed sender at deploy time.

use serde_json::json;

pub struct Mailer {
    api_key: Option<String>,
    from_email: String,
    from_name: String,
    app_url: String,
    http: reqwest::Client,
}

impl Mailer {
    pub fn from_env(http: reqwest::Client) -> Mailer {
        let api_key = std::env::var("SENDGRID_API_KEY").ok().filter(|k| !k.is_empty());
        if api_key.is_none() {
            tracing::warn!(
                "SENDGRID_API_KEY not set — dev mode: magic links will be LOGGED, not emailed"
            );
        }
        Mailer {
            api_key,
            from_email: std::env::var("FROM_EMAIL")
                .unwrap_or_else(|_| "jinteki-rs@localhost".into()),
            from_name: std::env::var("FROM_NAME").unwrap_or_else(|_| "jinteki-rs".into()),
            app_url: std::env::var("APP_URL")
                .unwrap_or_else(|_| "http://localhost:7787".into())
                .trim_end_matches('/')
                .to_string(),
            http,
        }
    }

    pub fn link_for(&self, token: &str) -> String {
        format!("{}/auth/verify?token={}", self.app_url, token)
    }

    /// Send the sign-in link. Same copy whether the claim will upgrade or
    /// merge — the email must not disclose whether the address has an
    /// account (§4.3). Failure is logged, never propagated: the claim row
    /// stays valid and the user just asks for a fresh link.
    pub async fn send_magic_link(&self, email: &str, token: &str) {
        let link = self.link_for(token);
        let Some(key) = &self.api_key else {
            tracing::info!("DEV MODE magic link for {email}: {link}");
            return;
        };
        let subject = "Sign in to jinteki-rs";
        let text = format!(
            "Click below to sign in:\n\n{link}\n\nThis link expires in 30 minutes. \
             If you didn't request this, ignore this email — nothing happens without clicking it.\n"
        );
        let html = format!(
            "<p>Click below to sign in:</p>\
             <p><a href=\"{link}\">Sign in</a></p>\
             <p>This link expires in 30 minutes. If you didn't request this, ignore \
             this email — nothing happens without clicking it.</p>"
        );
        let body = json!({
            "personalizations": [{"to": [{"email": email}]}],
            "from": {"email": self.from_email, "name": self.from_name},
            "subject": subject,
            "content": [
                {"type": "text/plain", "value": text},
                {"type": "text/html", "value": html}
            ]
        });
        let res = self
            .http
            .post("https://api.sendgrid.com/v3/mail/send")
            .bearer_auth(key)
            .json(&body)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => {
                tracing::info!("magic link sent (sendgrid {})", r.status());
            }
            Ok(r) => {
                tracing::error!("sendgrid refused the send: {}", r.status());
            }
            Err(e) => {
                tracing::error!("sendgrid send failed: {e}");
            }
        }
    }
}
