use payment_agent_failure_loop::{decide_risk, InfraiClient, PaymentEvent, RiskAction};

#[tokio::main]
async fn main() {
    let event = PaymentEvent { payment_id: "pay_demo_001".into(), amount_cents: 100_000, currency: "USD".into(), customer_id: "cus_demo".into() };
    let action = decide_risk(&event, 0);
    println!("payment {} -> {:?}", event.payment_id, action);
    if matches!(action, RiskAction::Review | RiskAction::Decline) {
        match InfraiClient::from_env() {
            Ok(client) => match client.capture_failure(&event, "risk action requires an audit event").await {
                Ok(_) => println!("audit event captured"),
                Err(error) => eprintln!("audit capture failed: {error}"),
            },
            Err(error) => eprintln!("audit capture skipped: {error}"),
        }
    }
}
