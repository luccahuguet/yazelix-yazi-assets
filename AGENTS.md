# Agent Guidelines

Shared Yazelix agent workflow and release policy live in the main repo:

- https://github.com/luccahuguet/yazelix/blob/main/AGENTS.md
- In sibling local checkouts, read `../yazelix/AGENTS.md` first

Only Yazelix Yazi Assets-specific guidance belongs here.

## Local Scope

- This repo owns reusable Yazi flavors, reusable plugins, and the Yazelix Starship prompt asset.
- Main Yazelix owns managed sidebar/editor orchestration plugins and session policy.
- Preserve upstream plugin license files when refreshing vendored plugins.

## Local Commands

- `nix build .#yazelix_yazi_assets --no-link`

## Integration Notes

Main Yazelix consumes this repo through its flake input and copies the assets into the managed runtime.
