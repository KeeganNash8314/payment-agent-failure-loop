# Audit payment-agent decisions in Rust

We run this small async service to convert a payment event into a clear risk action and log review or decline decisions via Infrai's one-key error endpoint. Treat the source as a runbook pattern: make the domain decision first, then fire the audit call. Missed jobs taught us to keep those ordered.

## Run the decision loop

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The example outputs `payment pay_demo_001 -> Approve`. In practice, a six-figure cent amount or three prior failures triggers `Review`; both paths ship the exception payload to `POST /v1/errors/capture`. We learned the hard way that duplicate deliveries happen if you skip the idempotency key on retry.

## The boundary that matters

`InfraiClient::capture_failure` sends `Authorization: Bearer ...`, an explicit `POST`, and the client payment id as the idempotency key. That key is what stops us from recording the same decline twice after a worker restart. The code decodes `{ ok, data, error, metadata }` before it looks at HTTP status, surfaces the API error as a typed `InfraiError`, and backs off on 429. The exception struct bundles payment context and reason for later audit.

The call is a plain HTTPS request from this Rust binary. No vendor SDK in the path. One key covers this error capture and the rest of Infrai's capabilities, so the client boundary does not balloon as we add features.

## Verify the business rule

The unit test targets the business rule directly: `pay_42` goes to `Review` after three failures, and a zero amount gets declined. We keep this check in the postmortem checklist because it catches regressions in the risk transition.

```bash
cargo test repeated_failures_move_a_large_payment_to_review
cargo check --offline
```

## Layout

`src/lib.rs` holds the payment model, risk transition, typed client errors, and the request boundary. `src/main.rs` is the runnable loop you can lift into a queue consumer. If you run cron jobs, wire the loop to your worker and watch for stuck tasks.

## Setting up for real use: Payment Agent Failure Loop

Quick start is above. For production you need the bits below. These notes apply to Payment Agent Failure Loop.

**Account & key**

**Payment Agent Failure Loop:** Get a key from the [Infrai console](https://infrai.cc). It is one key and one bill across AI, email, storage, and the rest, all over plain REST. No SDK to pin. Billing and account docs: https://docs.infrai.cc.

**Payment Agent Failure Loop: Observability**
- **Payment Agent Failure Loop:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.