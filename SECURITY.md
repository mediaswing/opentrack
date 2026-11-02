# Security Policy

## Supported versions

Security fixes go into the latest release only. Please check that a problem
still happens with the newest version from the
[releases page](https://github.com/mediaswing/opentrack/releases) before
reporting it.

| Version | Supported |
| --- | --- |
| 1.0.x (latest) | Yes |
| Older | No |

## Reporting a vulnerability

**Please don't report security problems in public issues, discussions or pull
requests.**

Report them privately through GitHub instead:

1. Go to the repository's [**Security** tab](https://github.com/mediaswing/opentrack/security).
2. Choose **Report a vulnerability**.
3. Fill in the form.

Only the maintainer can see the report. Please include:

- the version of Open Budget Tracker and your operating system
- the storage backend you were using (SQLite or MariaDB/MySQL)
- what the problem is and what an attacker could do with it
- steps to reproduce it, or a proof of concept
- any logs that help, with personal data removed (`output.log` is in the data
  folder; see [Files](README.md#files))

## What happens next

This is a project maintained in spare time, so these are aims rather than
promises:

- You should get a reply within **7 days** confirming the report was received.
- After looking into it, I'll tell you whether it's accepted as a
  vulnerability, and roughly when a fix is likely.
- Fixes are released as a new version, with the problem described in
  `CHANGELOG.md` and a GitHub security advisory once the fix is out.
- You'll be credited in the advisory unless you'd rather not be.

Please give a reasonable chance to release a fix before telling anyone else
about the problem.

## Scope

In scope:

- the `opentrack` application itself (the code in this repository)
- the `.pkg`, `.deb` and `.msi` packages it builds and that are published on
  GitHub releases
- the GitHub Actions workflows that build and publish them

Out of scope: vulnerabilities in third-party dependencies (please report those
to the project concerned, though a heads-up here is welcome if Open Budget
Tracker is affected), and problems that need someone to already control your
user account or computer.

## Known limitations

These are documented behaviours, not vulnerabilities, so there's no need to
report them:

- **The MariaDB/MySQL password is stored in plain text** in `config.json` in
  the data folder. Anyone who can read your user files can read it. Use a
  database account that can only reach the budget database.
- **Connections to MariaDB/MySQL are not encrypted** by the app. Use a server
  on the same machine or a trusted network, or tunnel the connection (for
  example over SSH).
- **The release packages are not signed** with a trusted certificate. The macOS
  app is only ad hoc signed, and macOS Gatekeeper and Windows SmartScreen warn
  about downloaded copies. Download releases only from this repository's
  releases page.
- **The update check** contacts `api.github.com` at startup (it can be turned
  off under **Settings → Updates**). It only shows a link to the new release;
  nothing is downloaded or installed automatically.
- The SQLite database and `output.log` are ordinary files in the data folder and
  are not encrypted.
