# Contributing

This repository holds the planned Thallium file manager. No source exists yet.

## Sign your commits (DCO, not a CLA)

Thallium uses the [Developer Certificate of Origin](https://developercertificate.org/).
There is no CLA to sign and no copyright assignment: you keep your copyright,
and you certify that you have the right to submit the work under this
repository's licence.

Every commit must carry a `Signed-off-by` line matching the author:

```sh
git commit -s -m "fix(scope): describe the change"
```

To sign off work you have already committed:

```sh
git rebase --signoff <base>
```

A bot checks this on every pull request. It is the only thing that will block
a pull request mechanically.

## Commit messages

Conventional Commits, imperative mood, 72 characters or fewer on the subject:

```
<type>(<scope>): <description>

[body: explain WHY, wrap at 72 columns]
```

Types: `feat` `fix` `perf` `refactor` `chore` `docs` `test` `ci` `revert`.

The body is where the value is. Explain why the change is correct, not what
the diff already shows — most of this project's commit bodies exist to stop a
future reader from "simplifying" a decision back into a bug.

## Pull requests

Say what breaks if the change is wrong. If a reviewer has to reconstruct the
failure mode from the diff, the description is too short.

Keep unrelated changes in separate commits. A packaging change and a rice
change in one commit cannot be reverted independently, and packaging changes
reach every installed machine.

## Code of conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
Report unacceptable behaviour to <36neelansh@gmail.com>.

## Security

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).
