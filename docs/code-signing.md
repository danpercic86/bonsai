# Code signing (Windows) — investigation, not implemented

Status: documentation only. No `tauri.conf.json` changes, no CI step, and no certificate
acquisition happen in this pass (per `docs/contracts/P2-followups.md` §6.3). This file is the seed
of a future P3 contract once a certificate is available.

## What's needed

A code-signing certificate for Windows: either a traditional `.pfx`/`.p12` file + password, or a
hardware-token/cloud-HSM certificate (e.g. Azure Trusted Signing, DigiCert KeyLocker for EV certs).
An EV or reputation-backed cert is what makes unsigned-publisher SmartScreen warnings disappear
quickly on modern Windows.

Options, ranked by typical friction:

1. **Azure Trusted Signing** (cloud, per-signature billing, no hardware token, Microsoft's
   recommended path for 2025+) — **recommended** if the goal is the lowest-friction setup for a
   small team.
2. **Traditional OV certificate** (`.pfx`) from a CA (DigiCert, Sectigo, SSL.com) — cheaper, but
   still triggers SmartScreen warnings until the certificate builds reputation over time.
3. **EV certificate** (hardware token / HSM) — best SmartScreen behavior immediately, highest
   cost/friction (typically requires a physical token or managed HSM).

## Tauri config touchpoints (once a certificate exists)

- `tauri.conf.json` → `bundle.windows.certificateThumbprint` for a locally-installed cert, or
  `bundle.windows.signCommand` for a custom `signtool` invocation — required for cloud/HSM-based
  signing (e.g. Azure Trusted Signing) which doesn't use a local `.pfx` file.
- `bundle.windows.digestAlgorithm` — set to `sha256`.
- `bundle.windows.timestampUrl` — e.g. `http://timestamp.digicert.com`, so the signature remains
  valid after the certificate itself expires.
- Local `.pfx` signing additionally needs the certificate password supplied via an environment
  variable at build time. Note: Tauri's `TAURI_SIGNING_PRIVATE_KEY`-style variables are for the
  **updater** signing feature (a separate, Ed25519-based mechanism), not Authenticode/Windows code
  signing — verify the exact variable name/mechanism against the current Tauri v2 docs at
  implementation time, since this is the part most likely to have shifted since this was written.

## Build/CI implication

Signing must happen on the machine running `pnpm tauri build`, or via a signing step Tauri shells
out to (`signCommand`). A cloud-HSM path (Azure Trusted Signing) avoids ever storing the private
key on the build machine, which is the safer default recommendation for a small team without
dedicated HSM infrastructure.

## Not implemented now

No `tauri.conf.json` edit, no CI step, no certificate acquisition — this cannot be done
autonomously since it requires a user-provided (and paid) certificate. Revisit once the user has
chosen and obtained a certificate; at that point this document becomes the seed of a proper P3
contract with the concrete config diff.

## Decision needed from the user

Pick one of the three options above (or propose an alternative) before any future code-signing
implementation milestone is scheduled. No native testing is needed for this decision — it is a
choice, not a build artifact.
