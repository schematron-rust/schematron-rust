# Security policy

## Supported versions

Security fixes are made on the latest released `0.x` line and on `main`.
Older minor releases are not maintained as separate security branches.

## Reporting a vulnerability

Do not open a public issue for a vulnerability that has not been disclosed.
Email `joel@joelparkerhenderson.com` with the subject
`schematron-rust security report` and include:

- the affected version or commit;
- the smallest reproduction available;
- the expected impact;
- whether the issue is already public; and
- a safe way to contact you for follow-up.

The maintainer will acknowledge the report, investigate it, and coordinate a
fix and disclosure. This volunteer project does not promise a response-time
SLA. Reports about malformed-input panics, resource exhaustion, unintended
file or network access, and incorrect validation that can produce a false
clean result are all in scope.

General correctness and feature reports belong in the public issue tracker.
