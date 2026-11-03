# Security

## Reporting a vulnerability

Please **do not** open a public issue for a security problem. Use GitHub's
[private security advisory](https://github.com/dirtTW/wa-desk/security/advisories/new)
form instead, or contact the maintainer directly.

Expect an acknowledgement within a few days. This is a hobby project with no
paid security response, so please be patient.

## Threat model

WA Desk is a local desktop client. What it protects:

- **Message content** — end-to-end encrypted by `whatsapp-rust` / `wacore-libsignal`
  (Signal protocol). This app never sees plaintext on the wire and never
  re-encrypts anything itself.
- **The linked device credentials** — the Noise/Signal session keys live in a
  local SQLite store (`whatsapp.db`) under the app data directory.
- **The optional AI API key** — encrypted at rest with AES-256-GCM under a
  random 32-byte key in `ai.key` (mode `0600` on Unix).

What it explicitly does **not** protect against:

- Someone with read access to your user profile — the store and the encryption
  key sit in the same directory, so full-disk encryption is the real defence.
- Malicious content inside a message: it is treated as untrusted and escaped,
  but a bug there is a real risk class.
- WhatsApp/Meta themselves, or a phone that is compromised.

## Audit — 2026-11-03, before the repository went public

| Area | Result |
| --- | --- |
| Secret-shaped strings in tracked files (`sk-…`, `ghp_…`, `github_pat_…`, `AKIA…`, `-----BEGIN … PRIVATE KEY`, `AIza…`, `xox…`) | **none** |
| The same patterns over the full git history | **none** |
| Commit author / committer emails | all `@users.noreply.github.com` (noreply) |
| Sensitive files tracked by git (`*.db`, `ai.json`, `ai.key`, `contacts.json`, `chats.json`, `.env`) | **none** — all covered by `.gitignore` |
| Hard-coded credentials | **none** |
| Dependency advisories (`cargo audit`, 681 crates) | 1 vulnerability + 6 unmaintained/unsound + 1 yanked — see below |
| Content Security Policy | was `null`; **now set** and verified with 0 violations |
| Untrusted message content | escaped before `innerHTML` — `bodyHtml()` escapes first, and there are 151 `escapeHtml` call sites |
| External links | `open_link` accepts only `http://` / `https://`, so a crafted message cannot spawn arbitrary commands |
| WebView2 permissions | only microphone and camera are auto-allowed; every other permission keeps the default (deny) |
| `unsafe` blocks | only Win32 / COM interop — the single-instance mutex, the WebView2 permission handler, and setting the WebView2 env var before any thread starts |
| Process execution | no `Command::new` / `std::process` in this project's own code |
| Update channel | minisign-signed; the app verifies every download against the public key pinned in `tauri.conf.json` |

### Dependency advisories

`cargo audit` is clean apart from advisories that need an upstream release.
They are recorded with a rationale in [`.cargo/audit.toml`](.cargo/audit.toml),
and CI runs `cargo audit` on every push.

- **`RUSTSEC-2026-0387` — `crypto-bigint` 0.5.5**, timing variability in
  constant-time operations. Reachable only through the VoIP stack
  (`crypto-bigint` ← `elliptic-curve` ← `p256`/`p384` ← `webrtc-dtls` ←
  `whatsapp-rust`), **not** the Signal end-to-end path. Exploiting it needs
  local timing measurements against a live call. Cleared by
  `elliptic-curve` 0.14 / `crypto-bigint` 0.7, which needs an upstream
  `webrtc-dtls` release.
- Unmaintained / unsound / yanked, all transitive, none with a known
  vulnerability: `bincode` 1.x, `proc-macro-error`, `zbus_names`, `zvariant`,
  `zvariant_derive` (Linux desktop integration), `glib` 0.18 (Linux only), and
  a yanked `async-executor`.

### Residual risks

- **The AI API key and the WhatsApp store share a directory.** Full-disk
  encryption is what protects them at rest; a per-file ACL is not set on
  Windows beyond the user profile's defaults.
- **`script-src 'unsafe-inline'`** is still required by five inline `onerror`
  handlers (avatar fallbacks). The CSP therefore blocks `object`/`frame` and
  restricts `connect-src`, but does not stop injected inline script. Moving
  those handlers to `addEventListener` would let `script-src` drop to `'self'`.
- **The updater trusts GitHub.** A compromised release on GitHub would be
  installed, because the signature check only proves the artefact came from the
  project's own signing key.
- **This code is AI-written and has not had a professional audit.** The checks
  above are the author's own, made with the tools in this repository.
