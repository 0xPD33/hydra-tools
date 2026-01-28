{
  description = "Hydra Tools - Agent utilities (hydra-mail, hydra-wt, hydralph, hydra-orchestrator)";

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

        # --- Crane Build System ---
        craneLib = crane.mkLib pkgs;

        # ============================================================
        # HYDRA-MAIL
        # ============================================================
        mailBuildInputs = [
          pkgs.openssl
          pkgs.sqlite
          pkgs.pkg-config
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        mailEnvVars = {
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

        mailCommonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ./hydra-mail);
          buildInputs = mailBuildInputs;
        } // mailEnvVars;

        mailCargoArtifacts = craneLib.buildDepsOnly mailCommonArgs;

        hydra-mail-pkg = craneLib.buildPackage (mailCommonArgs // {
          cargoArtifacts = mailCargoArtifacts;
          pname = "hydra-mail";
        });

        # ============================================================
        # HYDRA-OBSERVER (WIP - depends on mascots, not ready yet)
        # ============================================================
        # Note: hydra-observer is a thin integration layer.
        # Most GPU/Wayland deps come from the mascots crate.
        # We still need these for linking when building with mascots.
        # DISABLED UNTIL MASCOTS IS READY
        observerBuildInputs = [
          pkgs.pkg-config
          # Wayland (needed for linking with mascots)
          pkgs.wayland
          pkgs.wayland-protocols
          pkgs.libxkbcommon
          # Vulkan/GPU
          pkgs.vulkan-loader
          pkgs.vulkan-headers
          pkgs.shaderc
          # X11 fallback
          pkgs.xorg.libX11
          pkgs.xorg.libXcursor
          pkgs.xorg.libXrandr
          pkgs.xorg.libXi
          pkgs.xorg.libxcb
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        observerEnvVars = {
          LD_LIBRARY_PATH = lib.makeLibraryPath ([
            pkgs.wayland
            pkgs.libxkbcommon
            pkgs.vulkan-loader
            pkgs.xorg.libX11
            pkgs.xorg.libXcursor
            pkgs.xorg.libXrandr
            pkgs.xorg.libXi
            pkgs.xorg.libxcb
          ]);
          VULKAN_SDK = "${pkgs.vulkan-headers}";
          VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
        };

        observerCommonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ./hydra-observer);
          buildInputs = observerBuildInputs;
          nativeBuildInputs = [ pkgs.pkg-config ];
        } // observerEnvVars;

        observerCargoArtifacts = craneLib.buildDepsOnly observerCommonArgs;

        hydra-observer-pkg = craneLib.buildPackage (observerCommonArgs // {
          cargoArtifacts = observerCargoArtifacts;
          pname = "hydra-observer";
        });

        # ============================================================
        # HYDRA-WT (Worktree Manager)
        # ============================================================
        wtCommonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ./hydra-wt);
          buildInputs = lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

        wtCargoArtifacts = craneLib.buildDepsOnly wtCommonArgs;

        hydra-wt-pkg = craneLib.buildPackage (wtCommonArgs // {
          cargoArtifacts = wtCargoArtifacts;
          pname = "hydra-wt";
        });

        # ============================================================
        # HYDRA-ORCHESTRATOR (Session management library)
        # ============================================================
        # Uses workspace root since hydra-cli depends on it via path
        workspaceCommonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          buildInputs = lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

        orchestratorCargoArtifacts = craneLib.buildDepsOnly (workspaceCommonArgs // {
          pname = "hydra-orchestrator-deps";
          cargoExtraArgs = "--package hydra-orchestrator";
        });

        # ============================================================
        # HYDRA-CLI (CLI wrapper, depends on hydra-orchestrator)
        # ============================================================
        cliCargoArtifacts = craneLib.buildDepsOnly (workspaceCommonArgs // {
          pname = "hydra-cli-deps";
          cargoExtraArgs = "--package hydra-cli";
        });

        hydra-cli-pkg = craneLib.buildPackage (workspaceCommonArgs // {
          cargoArtifacts = cliCargoArtifacts;
          pname = "hydra-cli";
          cargoExtraArgs = "--package hydra-cli";
        });

      in
      {
        # --- Packages ---
        packages = {
          default = hydra-mail-pkg;
          hydra-mail = hydra-mail-pkg;
          # hydra-observer = hydra-observer-pkg;  # WIP - depends on mascots
          hydra-wt = hydra-wt-pkg;
          hydra-cli = hydra-cli-pkg;
        };

        # --- Apps ---
        apps = {
          default = flake-utils.lib.mkApp { drv = hydra-mail-pkg; };
          hydra-mail = flake-utils.lib.mkApp { drv = hydra-mail-pkg; };
          # hydra-observer = flake-utils.lib.mkApp { drv = hydra-observer-pkg; };  # WIP
          hydra-wt = flake-utils.lib.mkApp { drv = hydra-wt-pkg; };
          hydra = flake-utils.lib.mkApp { drv = hydra-cli-pkg; };
        };

        # --- Checks ---
        checks = {
          hydra-mail = craneLib.cargoClippy (mailCommonArgs // {
            cargoArtifacts = mailCargoArtifacts;
          });
          # hydra-observer = craneLib.cargoClippy (observerCommonArgs // {  # WIP
          #   cargoArtifacts = observerCargoArtifacts;
          # });
          hydra-wt = craneLib.cargoClippy (wtCommonArgs // {
            cargoArtifacts = wtCargoArtifacts;
          });
          hydra-orchestrator = craneLib.cargoClippy (workspaceCommonArgs // {
            cargoArtifacts = orchestratorCargoArtifacts;
            cargoExtraArgs = "--package hydra-orchestrator";
          });
          hydra-cli = craneLib.cargoClippy (workspaceCommonArgs // {
            cargoArtifacts = cliCargoArtifacts;
            cargoExtraArgs = "--package hydra-cli";
          });
        };

        # --- Development Shell ---
        # `nix develop` will drop you into this shell with all dependencies.
        devShells.default = pkgs.mkShell (mailEnvVars // observerEnvVars // {
          name = "hydra-tools-dev";

          nativeBuildInputs = [
            toolchain
            pkgs.pkg-config
            pkgs.clang
            pkgs.cmake
          ] ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.mold ];

          buildInputs = mailBuildInputs ++ observerBuildInputs ++ [
            pkgs.tree-sitter
            pkgs.cargo-dist
            pkgs.jq
            pkgs.fd
            pkgs.ripgrep
            pkgs.bat
            # Include built binaries for testing
            hydra-mail-pkg
            hydra-wt-pkg
            # hydra-cli-pkg  # TODO: Add back after generating Cargo.lock
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath ([
            pkgs.openssl
            pkgs.sqlite
            pkgs.wayland
            pkgs.libxkbcommon
            pkgs.vulkan-loader
            pkgs.xorg.libX11
            pkgs.xorg.libXcursor
            pkgs.xorg.libXrandr
            pkgs.xorg.libXi
            pkgs.xorg.libxcb
          ]);

          shellHook = ''
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
            export RUST_BACKTRACE=1
            export VK_LAYER_PATH="${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d"

            echo "--- Hydra Tools Development Environment ---"
            echo "Available packages: hydra-mail, hydra-wt, hydra-cli"
            echo "Build with: nix build .#hydra-mail (or hydra-wt, hydra-cli)"
          '';
        });
      });
}

