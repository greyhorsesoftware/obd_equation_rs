# Security

This crate evaluates equation strings from PID catalogs and user-supplied CSV
files; it performs no I/O and no network access. The main risk surface is a
malformed or hostile equation causing a panic or unbounded work in a host
application.

If you find such a case, or anything else you believe is a security issue,
email **info@greyhorsesoftware.com** rather than opening a public issue. Include
the equation and the bytes that trigger it. We aim to acknowledge within a
week.
