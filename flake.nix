{
  description = "An FHS shell with conda and cuda.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    naersk,
    fenix,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
      imports = [];
      perSystem = {system, ...}: let
        pkgs = import nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            cudaSupport = true;
            allowUnfreePredicate = pkg: true;
            acceptLicense = true;
          };
        };
        defaultDeps = [
          pkgs.ruff
          pkgs.nodejs
          pkgs.pyright
          pkgs.jq
          pkgs.uv
        ];
        toolchain = with fenix.packages.${system};
          combine [
            complete.cargo
            complete.rustc
            complete.clippy-preview
            complete.llvm-tools-preview
            complete.rust-analyzer-preview
            complete.rustfmt-preview
            complete.miri-preview
            targets."aarch64-unknown-linux-gnu".latest.rust-std
            targets."aarch64-unknown-linux-musl".latest.rust-std
            targets."x86_64-unknown-linux-gnu".latest.rust-std
            targets."x86_64-unknown-linux-musl".latest.rust-std
          ];

        kernel = pkgs.linux_6_10.override {
          argsOverride = rec {
            src = pkgs.fetchurl {
              url = "mirror://kernel/linux/kernel/v6.x/linux-${version}.tar.xz";
              sha256 = "sha256-5ofnNbXrnvttZ7QkM8k/yRGBBqmVUU8GJlKHO16Am80=";
            };
            version = "6.10.10";
            modDirVersion = "6.10.10";
          };
        };
        nvidia-x11 = import /home/ks/configs/zappacosta/dotfiles/overlays/nvidia-x11 {
          inherit
            (pkgs)
            lib
            callPackage
            fetchFromGitHub
            fetchgit
            fetchpatch
            stdenv
            pkgsi686Linux
            ;
          kernel = kernel;
        };
        nvidia-p2p = nvidia-x11.p2p;
        cudaDeps = with pkgs; [
          autoconf
          cmake # for triton build from src
          cudaPackages_12_1.cudatoolkit
          curl
          file
          freeglut
          gcc
          git
          gitRepo
          gnumake
          gnupg
          gperf
          libaio
          libGL
          libGLU
          libselinux
          libxml2
          m4
          ncurses5
          nvidia-p2p
          pkg-config # for triton build from src
          pkgs.acl
          pkgs.attr
          pkgs.bzip2
          pkgs.cmake
          pkgs.cudaPackages_12_1.cuda_cudart
          pkgs.cudaPackages_12_1.cuda_cudart.static
          pkgs.cudaPackages_12_1.cudatoolkit
          pkgs.cudaPackages_12_1.cudnn
          pkgs.cudaPackages_12_1.nccl
          pkgs.cudaPackages_12_1.nccl-tests
          pkgs.cudaPackages_12_1.nvidia_fs
          pkgs.curl
          pkgs.expat
          pkgs.file
          pkgs.fuse3
          pkgs.glibc
          pkgs.glibc_multi
          pkgs.icu
          pkgs.libaio
          pkgs.libsodium
          pkgs.libssh
          pkgs.libxml2
          pkgs.llvm_18
          pkgs.nss
          pkgs.openssl
          pkgs.pkg-config
          pkgs.python311Packages.triton
          pkgs.pythonManylinuxPackages.manylinux2014Package
          pkgs.stdenv.cc.cc
          pkgs.systemd
          pkgs.util-linux
          pkgs.vulkan-headers
          pkgs.vulkan-loader
          pkgs.vulkan-tools
          pkgs.xorg.libX11
          pkgs.xz
          pkgs.zlib
          pkgs.zstd
          procps
          stdenv.cc
          gcc
          unzip
          util-linux
          wget
          xorg.libICE
          xorg.libSM
          xorg.libX11
          xorg.libXext
          xorg.libXi
          xorg.libXmu
          xorg.libXrandr
          xorg.libXrender
          xorg.libXv
          binutils
          zlib

          # rust deps
          zig
          toolchain
          glib
          glibc
          gcc
          pkg-config
          clang
          llvmPackages_19.bintools
          llvmPackages_19.stdenv
          lld_19
          llvm_18 # for triton build from src
          libiconv
          openssl
          openssl.dev
          util-linux
          libcxx
        ];
        NIX_LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath cudaDeps;
      in {
        _module.args = {inherit pkgs;};
        legacyPackages = pkgs;
        devShells = {
          default = (pkgs.mkShell.override {stdenv = pkgs.gccStdenv;}) {
            name = "fwj-env";
            buildInputs =
              [
                pkgs.python311Packages.virtualenv
                pkgs.python311Packages.venvShellHook
                pkgs.pkg-config # Add pkg-config to buildInputs
                pkgs.openssl # Add OpenSSL explicitly
                pkgs.libiconv
                pkgs.openssl.dev
              ]
              ++ defaultDeps
              ++ cudaDeps;
            packages = defaultDeps ++ cudaDeps;
            NIX_LD_LIBRARY_PATH = NIX_LD_LIBRARY_PATH;
            LD_LIBRARY_PATH = NIX_LD_LIBRARY_PATH;
            HF_HOME = "/shelf/hf_home";
            HF_TOKEN = builtins.getEnv "HF_TOKEN";
            NVCC_APPEND_FLAGS = "-L${pkgs.cudaPackages_12_1.cuda_cudart.static}/lib";
            TORCH_CUDA_ARCH_LIST = "8.9"; # support for 4090 not to compile useless compatibilities
            TRITON_LIBCUDA_PATH = "${nvidia-p2p}/lib/libcuda.so";
            NIX_LD = pkgs.lib.fileContents "${pkgs.stdenv.cc}/nix-support/dynamic-linker";
            CUDA_PATH = "${pkgs.cudaPackages_12_1.cudatoolkit}";
            TORCH_USE_CUDA_DSA = "1";
            CUDA_VISIBLE_DEVICES = "0,1,2";

            # Add pkg-config related environment variables
            PKG_CONFIG_PATH = pkgs.lib.makeSearchPath "lib/pkgconfig" (defaultDeps ++ cudaDeps);
            PKG_CONFIG_SYSROOT_DIR = "/";

            shellHook = ''
              set -eu
              export LD_LIBRARY_PATH=$NIX_LD_LIBRARY_PATH
              export TAILSCALE_IP=$(tailscale ip -4 2>/dev/null)
              export HF_TOKEN=${builtins.getEnv "HF_TOKEN"}
              export CUDA_PATH="${pkgs.cudaPackages_12_1.cudatoolkit}"
              export HF_HOME="/shelf/hf_home"
              export OMP_NUM_THREADS=32

              # Set up pkg-config wrapper for cross-compilation
              export PKG_CONFIG="${pkgs.pkg-config}/bin/pkg-config"
              export PKG_CONFIG_PATH="$PKG_CONFIG_PATH"
              export PKG_CONFIG_SYSROOT_DIR="$PKG_CONFIG_SYSROOT_DIR"
            '';
          };
        };
      };
    };
}
