# Security policy

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.**

Report privately, by either route:

- **GitHub private vulnerability reporting** — preferred. Go to the
  [Security tab](https://github.com/thallium-os/thallium-files/security/advisories/new)
  and open a draft advisory. This keeps the report, the fix, and the CVE
  request in one place.
- **Email** — <36neelansh@gmail.com>. Use this if you cannot access GitHub, or
  if the issue is severe enough that you want it out of a web form.

Please include: what you can do with the bug, the version or commit you tested,
your distribution and kernel, and the smallest reproduction you have. A
proof-of-concept is welcome but not required — a clear description of the flaw
is enough to start.

## What to expect

| | |
|---|---|
| Acknowledgement | within 7 days |
| Initial assessment | within 14 days |
| Disclosure | coordinated, once a fix is available |

Thallium is maintained by one person. These are honest targets, not a
guaranteed SLA. If you have not heard back within the acknowledgement window,
assume the message was missed and send a follow-up — that is not a nuisance.

If a report is valid and you want credit, you will get it in the advisory and
the release notes. If you would rather stay anonymous, say so.

## Scope

In scope: Nothing yet. This repository is a reserved name with a README and no code.

When source lands, this section will name the real boundaries. Until then there is nothing here to attack, and a report against this repository is almost certainly meant for another one — see the list in `thallium`'s policy.

Out of scope: Everything, for now.

## Supported versions

Thallium is pre-1.0. Only the tip of `main` is supported; fixes are not
backported. The OTA archive serves one suite, `stable`, and it always carries
the newest release.
