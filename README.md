# yazelix-yazi-assets

Standalone Yazi flavor and plugin assets extracted from Yazelix

This repository exists for non-Yazelix users who want the reusable Yazi pieces without adopting the full Yazelix runtime. Regular Yazelix users do not need to install or configure this package directly; Yazelix wires it into the managed runtime

## Contents

- `flavors/` contains the bundled Yazelix Yazi flavor catalog
- `plugins/git.yazi/`, `plugins/lazygit.yazi/`, and `plugins/starship.yazi/` contain reusable Yazi plugins with their upstream license files
- `plugins/auto-layout.yazi/` contains the Yazelix-maintained Yazi auto-layout helper
- `yazelix_starship.toml` contains the Starship prompt config used by the Yazi integration
- `config_metadata/yazi_assets_manifest.toml` declares the packaged asset shape for consumers that need a stable manifest

Yazelix-specific sidebar/editor orchestration plugins remain in the main Yazelix repository because they depend on the managed pane/session contract

## Nix

Build the package:

```bash
nix build .#yazelix_yazi_assets
```

The package installs assets under:

```text
share/yazelix_yazi_assets/
```

That directory contains `flavors/`, `plugins/`, `yazelix_starship.toml`, and `config_metadata/yazi_assets_manifest.toml`
