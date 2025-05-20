# code-pick-recent-rs
Input for e.g. wofi to select a recently opened folder in vscode.

## Install
```bash
cargo install --path .
```

## Usage
Example with wofi:
```bash
~/.cargo/bin/code-pick-recent-rs -wW | wofi --dmenu | xargs -r code
```

I use it as a bind in my `hyperland.conf`:
```bash
bind = $mainMod SHIFT, C, exec, ~/.cargo/bin/code-pick-recent-rs -wW | wofi --dmenu | xargs -r code
```
