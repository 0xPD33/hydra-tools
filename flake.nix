{
  description = "Hydra Agent development environment";

  # --- Inputs ---
  # These are the external dependencies for our development environment.
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    # The rust-overlay provides up-to-date Rust toolchains.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    # Crane is a modern library for building Rust projects with Nix.
    # It provides fine-grained caching for extremely fast rebuilds.
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # --- Outputs ---
  # This section defines what the flake provides: packages, apps, shells, etc.
  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # --- Overlays ---
        # Overlays allow us to add or modify packages from nixpkgs.
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        lib = pkgs.lib;

        # --- Rust Toolchain ---
        # We pin the Rust toolchain declaratively using the rust-toolchain.toml file.
        # This ensures every developer uses the exact same Rust version.
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # --- Shared Dependencies ---
        # Common dependencies used by both build and development environments.
        sharedBuildInputs = [
          pkgs.openssl
          pkgs.sqlite
          pkgs.pkg-config
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          # Add macOS specific build dependencies here if needed.
          pkgs.libiconv
        ];

        # --- Shared Environment Variables ---
        # Common environment configuration for both build and development.
        sharedEnvVars = {
          LD_LIBRARY_PATH = lib.makeLibraryPath [
            pkgs.openssl
            pkgs.sqlite
          ];
          OPENSSL_STATIC = "0";
          OPENSSL_DIR = pkgs.openssl.dev;
          OPENSSL_INCLUDE_DIR = (
            pkgs.lib.makeSearchPathOutput "dev" "include" [ pkgs.openssl.dev ]
          ) + "/openssl";
        };

        # --- Crane Build System ---
        # This section defines how to build the `hydra-agent` package.
        craneLib = crane.mkLib pkgs;

        # Common arguments for all crane build steps.
        commonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          buildInputs = sharedBuildInputs;
        } // sharedEnvVars;

        # Step 1: Build dependencies only. This is the slow part and gets cached.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Step 2: Build the `hydra-agent` binary itself, using the cached dependencies.
        # This step is very fast.
        hydra-agent-pkg = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "hydra-agent";
        });

      in
      {
        # --- Default Package ---
        # `nix build .` will produce the hydra-agent binary.
        packages.default = hydra-agent-pkg;

        # --- Default App ---
        # `nix run .` will execute the hydra-agent binary.
        apps.default = flake-utils.lib.mkApp {
          drv = hydra-agent-pkg;
        };

        # --- Default Check ---
        # `nix flake check` will run `cargo check`.
        checks.default = craneLib.checkCargoPackage commonArgs;

        # --- Development Shell ---
        # `nix develop` will drop you into this shell.
        devShells.default = pkgs.mkShell (sharedEnvVars // {
          name = "hydra-agent-core-dev";

          # Tools used for BUILDING the project.
          nativeBuildInputs = [
            # The full Rust toolchain with cargo, clippy, etc.
            toolchain

            # C compiler and build tools required by some Rust crates.
            pkgs.pkg-config
            pkgs.clang
            pkgs.cmake
          ]
            # Use the 'mold' linker on Linux for much faster link times.
            ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.mold ];

          # Tools and libraries available at RUNTIME inside the shell.
          buildInputs = sharedBuildInputs ++ [
            # Code parsing tools.
            pkgs.tree-sitter

            # Packaging tools.
            pkgs.cargo-dist

            # General purpose CLI tools for a better developer experience.
            pkgs.jq
            pkgs.fd
            pkgs.ripgrep
            pkgs.bat
          ];

          # Environment variables for the development shell.
          shellHook = ''
            # Make rust-analyzer work seamlessly.
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
            export RUST_BACKTRACE=1

            echo "--- Hydra Agent Core Environment ---"
            echo "Rust toolchain and all dependencies are now available."
            echo "Run 'cargo build' or 'cargo run' to get started."
          '';
        });
      });
}

