---
name: zerant-backend
description: Secure API, service, persistence, validation, logging, and backend patterns for Zerant.
---
# Zerant Backend
Use when adding APIs, persistence, hosted metadata, issuer services, or verifier services.

Rules:
- Validate all inputs at trust boundaries.
- Separate handlers, business logic, and persistence where complexity warrants it.
- Standardize errors and structured logs; never log credential plaintext or secrets.
- Prefer public metadata services only: issuer keys, schemas, revocation state, verifier integration.
- Never make the backend the default holder credential store.
- Use database transactions for coupled writes and shared rate limiting for production APIs.
- Add replay/idempotency controls where requests can be retried.
- Add unit and integration tests around trust failures.
