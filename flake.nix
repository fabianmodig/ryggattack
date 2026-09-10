{
  description = "Back Snack development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forEachSystem (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              pkg-config
              alsa-lib
              udev
              vulkan-loader
              libxkbcommon
              wayland
              libx11
              libxcursor
              libxi
              libxrandr
            ];

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
              alsa-lib
              udev
              vulkan-loader
              libxkbcommon
              wayland
              libx11
              libxcursor
              libxi
              libxrandr
            ]);
          };
        });
    };
}
