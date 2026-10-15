# Security policy

## Supported versions

Only the latest version (the `main` branch and the latest release, if any) receives security fixes.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Use GitHub's private reporting instead:
open the **Security** tab of this repository and choose **Report a vulnerability**.

Useful details: the version or commit concerned, how to reproduce the problem, and what an attacker
could do with it.

This is a personal project, maintained in my spare time. I read reports as soon as I can, usually
within a few days, and I will tell you when a fix is published. Please give me a reasonable time to
fix the problem before disclosing it.

## Scope

The code of Plume, and in particular the security rules of `src/policy.rs` (which pages may be
displayed, which pop-ups, links, permissions and downloads are allowed). Discord's own web app and
Microsoft's WebView2 are out of scope: report those to their vendors.

A credential or personal data found in the repository or in its history is also worth reporting the
same way.
