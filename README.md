# code-pick-rs
Input for e.g. wofi for selecting from a list of recently opened folders/files or a list of workspaces to be opened in vscode.

## Install
```bash
cargo install --path .
```

## Usage
Example with wofi:
```bash
codep recent -a | wofi --dmenu | xargs -r code
```

I use it as a bind in my `hyperland.conf`:
```bash
bind = $mainMod SHIFT, C, exec, ~/.cargo/bin/codep recent -a | wofi --dmenu | xargs -r code
bindr = $mainMod&CTRL&SHIFT, C, exec, ~/.cargo/bin/codep workspaces -x 365 | wofi --dmenu | xargs -r code
```

`codep --help` for more info!

## Environment Variables

`CODEP_CONFIG_ROOT` (default: `~/.config/Code`) - Alternative config root
