# Security Policy

## Supported versions

RuJa is pre-1.0 alpha software. Security fixes are applied to the latest
`main` branch only — there are no backports to older releases.

## Reporting a vulnerability

If you discover a security vulnerability, **please do not open a public
issue**. Instead, report it privately:

1. Open a **private security advisory** on GitHub:
   [Report a vulnerability](https://github.com/flyingsquirrel0419/RuJa/security/advisories/new)
2. Or email the maintainer directly.

Please include:

- A description of the vulnerability and its impact
- A minimal reproduction (JS input that triggers it)
- The RuJa version / commit you tested against
- Your assessment of severity

You should receive an acknowledgement within 72 hours. Please do not
disclose the vulnerability publicly until a fix has been released.

## Scope

RuJa is a local-trust JavaScript engine intended for trusted input. It does
not implement a security sandbox for untrusted code. Vulnerabilities that
cause memory unsafety, panics, or crashes from crafted JS input are in scope;
the absence of an isolation sandbox itself is a known limitation (see
[docs/limitations.md](docs/limitations.md)).
