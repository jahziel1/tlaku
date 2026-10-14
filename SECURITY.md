# Security policy

English · [Español](SECURITY.es.md)

Tlaku controls machines that can burn, cut and start fires. For this project, **safety bugs are not
only software bugs**: some of them can hurt people or damage property. Please report them privately.

## How to report a vulnerability (privately)

Use GitHub's **Private vulnerability reporting**:
<https://github.com/jahziel1/tlaku/security/advisories/new>

1. Open the link above (a GitHub account is needed).
2. Describe what happens, how to reproduce it and, if you know, the machine and controller.
3. Do **not** open a public issue, discussion or pull request for it, and do not post it in forums
   until it is fixed.

Include as much as you can: Tlaku version, system, machine and controller (and firmware), the steps,
the console log and, if it is safe to share, the design or the G-code.

You will get an answer as soon as a maintainer can read it. This is a volunteer project without a
company behind it, so we cannot promise fixed response times, but laser-safety reports are handled
first (see below). We will keep you informed, fix the problem, publish an advisory and credit you if
you want.

## Laser safety: treated as urgent

A flaw that can do any of the following is treated as an **urgent vulnerability**, even if it needs no
attacker (for example, a plain bug that triggers it by itself):

- **Turn the laser on where it should not be on**: firing during travel moves, while framing with the
  laser meant to be off, after a pause, a stop, an alarm or a disconnection, or at a higher power
  than the one configured.
- **Fail to stop it**: Pause, Stop or an emergency stop that does not take effect, a stop that is
  reported as done when it was not confirmed, or a laser that stays on after an error or when the
  connection drops.
- **Move the machine unexpectedly**: sending motion without the confirmations Tlaku requires
  (pre-flight check, confirmation dialogs, the framing requirement), homing or unlocking without the
  user asking, wrong coordinates, directions or scale that make the head leave the work area, or
  starting a job from the phone control or the automation server without the required confirmations.

If you are unsure whether something counts, report it privately anyway.

Remember that Tlaku has **never been tested with a real laser** (only emulators), so these
bugs may exist. If you test on real hardware, follow the levels of the
[validation list](docs/validacion-hardware.md), use eye protection, scrap material, and keep your
hand on the emergency stop. If something unexpected happens during a test, stop the machine first and
report afterwards.

## Other security topics

These are also welcome as private reports:

- The **phone control** (local web server) and the **automation server** (local UDP): bypassing the
  key, reaching them from outside the allowed network, or starting a job without confirmation.
- Opening untrusted files (`.tlaku`, SVG, DXF, PDF, LightBurn projects, G-code, `.tlakubundle`,
  `.tlakumat`) in a way that crashes the app, runs code, reads or writes files it should not.
- The **updater** and the installers (signature checks, tampering).
- Dependencies with known vulnerabilities that affect Tlaku.

For ordinary bugs (without safety impact), use the
[bug report form](https://github.com/jahziel1/tlaku/issues/new?template=bug_report.yml).

## Supported versions

Tlaku has no stable release yet. Fixes go to the latest version on the `main` branch and to the latest
published release.
