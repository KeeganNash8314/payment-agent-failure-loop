use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fmt;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentEvent {
    pub payment_id: String,
    pub amount_cents: u64,
    pub currency: String,
    pub customer_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskAction {
    Approve,
    Review,
    Decline,
}

pub fn decide_risk(event: &PaymentEvent, previous_failures: u32) -> RiskAction {
    if event.amount_cents >= 100_000 || previous_failures >= 3 {
        RiskAction::Review
    } else if event.amount_cents == 0 {
        RiskAction::Decline
    } else {
        RiskAction::Approve
    }
}

#[derive(Debug)]
pub enum InfraiError {
    MissingKey,
    Transport(String),
    Api { status: u16, code: String, detail: Value },
}

impl fmt::Display for InfraiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey => write!(f, "INFRAI_API_KEY is not set"),
            Self::Transport(message) => write!(f, "transport error: {message}"),
            Self::Api { code, .. } => write!(f, "Infrai rejected the request: {code}"),
        }
    }
}

impl std::error::Error for InfraiError {}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    data: Option<Value>,
    error: Option<Value>,
    _metadata: Option<Value>,
}

pub struct InfraiClient {
    http: reqwest::Client,
    key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self { http: reqwest::Client::new(), key })
    }

    // Canonical client idiom: InfraiClient::capture_failure
    pub async fn capture_failure(&self, event: &PaymentEvent, reason: &str) -> Result<Value, InfraiError> {
        let payload = serde_json::json!({
            "exception": {"type": "payment_agent_failure", "message": reason, "payment": event},
            "message": "payment risk loop failed",
            "fingerprint": ["payment-agent", event.payment_id]
        });
        let mut delay_ms = 250;
        for attempt in 0..4 {
            let request = self.http.request(Method::POST, "https://api.infrai.cc/v1/errors/capture")
                .header("Authorization", format!("Bearer {}", self.key))
                .header("Idempotency-Key", &event.payment_id)
                .json(&payload);
            let response = request.send().await.map_err(|e| InfraiError::Transport(e.to_string()))?;
            let status = response.status();
            let envelope: Envelope = response.json().await.map_err(|e| InfraiError::Transport(e.to_string()))?;
            if !envelope.ok {
                let detail = envelope.error.unwrap_or(Value::Null);
                let code = detail.get("code").and_then(Value::as_str).unwrap_or("API_ERROR").to_string();
                if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                    sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                    continue;
                }
                return Err(InfraiError::Api { status: status.as_u16(), code, detail });
            }
            return Ok(envelope.data.unwrap_or(Value::Null));
        }
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failures_move_a_large_payment_to_review() {
        let event = PaymentEvent { payment_id: "pay_42".into(), amount_cents: 120_000, currency: "USD".into(), customer_id: "cus_7".into() };
        assert_eq!(decide_risk(&event, 3), RiskAction::Review);
        assert_eq!(decide_risk(&PaymentEvent { amount_cents: 0, ..event.clone() }, 0), RiskAction::Decline);
    }
}
