# Fast iteration: run WA Desk in dev mode so UI changes are live.
#
#   1. starts the static UI server (scripts/dev-server.mjs, port 1421) —
#      `apps/desktop/ui` is served from DISK, so a reload (Ctrl+R) shows UI
#      edits instantly, no binary rebuild. NOTE: in dev the app runs at
#      http://127.0.0.1:1421, in the release package at tauri.localhost;
#   2. runs `npx @tauri-apps/cli dev`, which compiles the app with the dev
#      profile (incremental, ~13 s after the first build) and launches it
#      against http://127.0.0.1:1421 (tauri.conf.json `build.devUrl`).
#
# Only one instance may own the WhatsApp session store: close the release
# WA Desk first (the script does it for you). Ctrl+C tears everything down.

Get-Process "WA Desk", wa-desktop -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

$server = Start-Process node -ArgumentList "./scripts/dev-server.mjs" -PassThru -WindowStyle Hidden
try {
  # devUrl comes in as a --config overlay so the shipped config stays clean.
  npx @tauri-apps/cli dev --config '{"build":{"devUrl":"http://127.0.0.1:1421"}}'
}
finally {
  $server | Stop-Process -Force -ErrorAction SilentlyContinue
  Get-Process "WA Desk", wa-desktop -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}
