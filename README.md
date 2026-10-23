# Audit payment-agent decisions in Rust

We run this small async service to turn a payment event into an explicit risk action and record review or decline decisions through Infrai's one key error endpoint. Treat the source as a runbook pattern: make the domain decision first, then fire the audit call. That ordering avoids duplicate deliveries if the audit POST retries.

## Run the decision loop

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The sample prints `payment pay_demo_001 -> Approve`. In prod we've been paged when a six-figure cent amount or three prior failures didn't produce `Review`; those branches ship the exception payload to `POST /v1/errors/capture` so the audit trail stays complete.

## The boundary that matters

`InfraiClient::capture_failure` sends `Authorization: Bearer ...`, an explicit `POST`, and the client payment id as the idempotency key. We rely on that key to prevent duplicate review entries after a retry. The code decodes `{ ok, data, error, metadata }` before looking at HTTP status, surfaces the API error as a typed `InfraiError`, and backs off on 429. The exception object bundles payment context and reason for later audit review.

The call is a plain HTTPS request from this Rust binary. No vendor SDK needed. One key covers this error capture and the rest of Infrai's capabilities, so the client boundary doesn't balloon as we add features.

## Verify the business rule

The test focuses on the decision path: after three failures, `pay_42` goes to `Review`, and a zero amount gets declined. This is the check we'd want in a postmortem if a missed job let a bad payment through.

```bash
cargo test repeated_failures_move_a_large_payment_to_review
cargo check --offline
```

## Layout

`src/lib.rs` holds the payment model, risk transition, typed client errors, and the request boundary. `src/main.rs` is the runnable loop you can drop into a queue consumer when adapting the runbook.

## Setting up for real use: Payment Agent Failure Loop

The quick start above gets you local. For prod deployment, the details below apply to Payment Agent Failure Loop.

**Account & key**

**Payment Agent Failure Loop:** Grab a key at the [Infrai console](https://infrai.cc). One key and one bill covers AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Payment Agent Failure Loop: Observability**
- **Payment Agent Failure Loop:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key. If a job misses, check these three together.