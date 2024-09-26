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

        supportedTargets = [
          "x86_64-unknown-linux-musl"
        ];

        buildTargets = {
          "x86_64-linux" = {
            crossSystemConfig = "x86_64-unknown-linux-gnu";
            rustTarget = "x86_64-unknown-linux-gnu";
          };

          "i686-linux" = {
            crossSystemConfig = "i686-unknown-linux-musl";
            rustTarget = "i686-unknown-linux-musl";
          };

          "aarch64-linux" = {
            crossSystemConfig = "aarch64-unknown-linux-musl";
            rustTarget = "aarch64-unknown-linux-musl";
          };

          "armv6l-linux" = {
            crossSystemConfig = "armv6l-unknown-linux-musleabihf";
            rustTarget = "arm-unknown-linux-musleabihf";
          };

          "x86_64-windows" = {
            crossSystemConfig = "x86_64-w64-mingw32";
            rustTarget = "x86_64-pc-windows-gnu";
            makeBuildPackageAttrs = pkgsCross: {
              depsBuildBuild = [
                pkgsCross.stdenv.cc
                pkgsCross.windows.pthreads
              ];
            };
          };
        };

        toolchain = with fenix.packages.${system};
          combine ([
              (
                if pkgs.stdenv.hostPlatform.config == buildTargets.${system}.crossSystemConfig
                then complete.toolchain
                else minimal.toolchain
              )
            ]
            ++ (
              builtins.map
              (target: targets.${target}.latest.rust-std)
              (builtins.attrValues (builtins.mapAttrs (name: value: value.rustTarget) buildTargets))
            ));

        buildInputs = with pkgs; [
          openssl
          util-linux
          glibc
          libcxx
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
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
        rustDeps = [pkgs.cargo pkgs.rustc pkgs.rustfmt pkgs.pre-commit pkgs.rustPackages.clippy];
        cudaDeps = with pkgs; [
          autoconf
          cmake # for triton build from src
          cudaPackages_12_1.cudatoolkit
          curl
          file
          freeglut
          gcc11
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
          lld_19
          llvm_18 # for triton build from src
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
          gcc11
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
        ];
        NIX_LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath cudaDeps;
      in {
        _module.args = {inherit pkgs;};
        legacyPackages = pkgs;
        packages.default =
          (
            naersk.lib.${system}.override {
              cargo = toolchain;
              rustc = toolchain;
            }
          )
          .buildPackage {
            src = ./.;
            LD_LIBRARY_PATH = NIX_LD_LIBRARY_PATH;
            CARGO_BUILD_TARGET = buildTargets.${system}.rustTarget;
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.gcc11}/bin/gcc";
          };

        devShells = {
          default = (pkgs.mkShell.override {stdenv = pkgs.gcc11Stdenv;}) {
            name = "fwj-env";
            buildInputs = [
              pkgs.python311Packages.virtualenv
              pkgs.python311Packages.venvShellHook
            ];
            packages = defaultDeps ++ cudaDeps ++ rustDeps;
            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
            NIX_LD_LIBRARY_PATH = NIX_LD_LIBRARY_PATH;
            HF_HOME = "/shelf/hf_home";
            HF_TOKEN = builtins.getEnv "HF_TOKEN";
            NVCC_APPEND_FLAGS = "-L${pkgs.cudaPackages_12_1.cuda_cudart.static}/lib";
            TORCH_CUDA_ARCH_LIST = "8.9"; # support for 4090 not to compile useless compatibilities
            TRITON_LIBCUDA_PATH = "${nvidia-p2p}/lib/libcuda.so";
            NIX_LD = pkgs.lib.fileContents "${pkgs.stdenv.cc}/nix-support/dynamic-linker";
            CUDA_PATH = "${pkgs.cudaPackages_12_1.cudatoolkit}";
            TORCH_USE_CUDA_DSA = "1";
            CUDA_VISIBLE_DEVICES = "0,1,2";
            shellHook = ''
              set -eu
              export LD_LIBRARY_PATH=$NIX_LD_LIBRARY_PATH
              export TAILSCALE_IP=$(tailscale ip -4 2>/dev/null)
              export HF_TOKEN=${builtins.getEnv "HF_TOKEN"}
              export CUDA_PATH="${pkgs.cudaPackages_12_1.cudatoolkit}"
              export HF_HOME="/shelf/hf_home"
              export OMP_NUM_THREADS=32
            '';
          };
        };
      };
    };
}
