{
  description = "sverilogparse";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      flake-utils,
      nixpkgs,
      rust-overlay,
    }:

    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [
          (import rust-overlay)
          (self: super: {
            rustToolchain = pkgs.symlinkJoin {
              name = "rust-toolchain";
              paths = [
                (super.rust-bin.stable.latest.minimal.override {
                  extensions = [
                    "clippy"
                    "rust-analyzer"
                    "rust-docs"
                    "rust-src"
                  ];
                })
                (super.rust-bin.selectLatestNightlyWith (toolchain: toolchain.rustfmt))
              ];
            };
          })
        ];

        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain # Rust compiler and cargo
            pkg-config # Build-time dependency finder
            cargo-deny # Check dependencies for security issues
            cargo-edit # Cargo subcommands: add, rm, upgrade
            cargo-watch # Auto-rebuild on file changes
            cargo-machete # Check for unused dependencies
          ];
        };
      }
    );
}
