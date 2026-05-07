{
  description = "Standalone Yazi flavor and plugin assets from Yazelix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };

          assets = pkgs.stdenvNoCC.mkDerivation {
            pname = "yazelix_yazi_assets";
            version = "0.1.0";
            src = pkgs.lib.cleanSource ./.;

            dontConfigure = true;
            dontBuild = true;

            installPhase = ''
              runHook preInstall

              install_root="$out/share/yazelix_yazi_assets"
              mkdir -p "$install_root"

              cp -R flavors "$install_root/flavors"
              cp -R plugins "$install_root/plugins"
              install -Dm644 yazelix_starship.toml "$install_root/yazelix_starship.toml"
              install -Dm644 README.md "$out/share/doc/yazelix_yazi_assets/README.md"
              install -Dm644 LICENSE "$out/share/doc/yazelix_yazi_assets/LICENSE"
              install -Dm644 config_metadata/vendored_yazi_plugins.toml "$out/share/yazelix_yazi_assets/config_metadata/vendored_yazi_plugins.toml"
              install -Dm644 config_metadata/vendored_yazi_plugin_patches/git.yazi.patch "$out/share/yazelix_yazi_assets/config_metadata/vendored_yazi_plugin_patches/git.yazi.patch"

              runHook postInstall
            '';

            doInstallCheck = true;
            nativeInstallCheckInputs = [
              pkgs.coreutils
              pkgs.findutils
              pkgs.gnugrep
            ];
            installCheckPhase = ''
              runHook preInstallCheck

              install_root="$out/share/yazelix_yazi_assets"
              test -f "$install_root/yazelix_starship.toml"
              test -f "$install_root/flavors/catppuccin-mocha.yazi/flavor.toml"
              test -f "$install_root/plugins/git.yazi/main.lua"
              test -f "$install_root/plugins/lazygit.yazi/main.lua"
              test -f "$install_root/plugins/starship.yazi/main.lua"
              test -f "$install_root/plugins/auto-layout.yazi/main.lua"

              flavor_count="$(find "$install_root/flavors" -name flavor.toml | wc -l | tr -d ' ')"
              test "$flavor_count" = "24"

              runHook postInstallCheck
            '';

            passthru = {
              assetsRoot = "share/yazelix_yazi_assets";
              flavorsPath = "share/yazelix_yazi_assets/flavors";
              pluginsPath = "share/yazelix_yazi_assets/plugins";
            };

            meta = {
              description = "Reusable Yazi flavor and plugin assets from Yazelix";
              license = pkgs.lib.licenses.mit;
              platforms = systems;
            };
          };
        in
        {
          default = assets;
          yazelix_yazi_assets = assets;
        }
      );

      checks = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          package = self.packages.${system}.yazelix_yazi_assets;
        in
        {
          install = package;
          asset_shape = pkgs.runCommand "yazelix-yazi-assets-shape" { } ''
            install_root="${package}/share/yazelix_yazi_assets"
            test -d "$install_root/flavors"
            test -d "$install_root/plugins"
            test -f "$install_root/plugins/git.yazi/main.lua"
            test -f "$install_root/plugins/starship.yazi/main.lua"
            test -f "$install_root/plugins/auto-layout.yazi/main.lua"
            touch "$out"
          '';
        }
      );
    };
}
