# User-level test SOPs

Procedures executed against the real windowed build by a human (today) or a computer-use agent (planned).

Fully black-box: OS input in, pixels out.

## SOP-001: demo sanity
1. `just run` — window opens, red box oscillates horizontally
2. `just swap-asset` in another terminal — box turns blue without restart
3. Edit `assets/config/demo_box.ron` speed to 8.0, save — box speeds up instantly
4. Press ` (backquote) — world inspector opens
