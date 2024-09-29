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
  }: let
    supportedSystems = ["x86_64-linux"]; #[ "x86_64-darwin" "aarch64-darwin"];
  in
    flake-parts.lib.mkFlake {inherit inputs;}
    {
      imports = [];

      systems = supportedSystems;

      perSystem = {system, ...}: let
        pkgs = import nixpkgs {
          system = "x86_64-linux";
          config = {
            allowUnfree = true;
            cudaSupport = true;
            allowUnfreePredicate = pkg: true;
            acceptLicense = true;
          };
        };

        pkgsStatic = pkgs.pkgsStatic;

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

        #   # darwinBuildInputs =
        #   #   with darwin.apple_sdk.frameworks;
        #   #   [
        #   #     Accelerate
        #   #     CoreVideo
        #   #     CoreGraphics
        #   #   ]
        #   #   ++ lib.optionals metalSupport [
        #   #     MetalKit
        #   #     MetalPerformanceShaders
        #   #   ];
        #   # ++ lib.optionals mklSupport [ mkl ]
        #   # ++ lib.optionals stdenv.hostPlatform.isDarwin darwinBuildInputs;

        cudaDeps = with pkgs; [
          nvidia-p2p
          cudaPackages_12_4.cuda_cudart
          cudaPackages_12_4.cuda_cudart.static
          cudaPackages_12_4.cudatoolkit
          cudaPackages_12_4.cudnn
          cudaPackages_12_4.nccl
          cudaPackages_12_4.nccl-tests
          cudaPackages_12_4.nvidia_fs
          cudaPackages_12_4.cuda_nvcc
          cudaPackages_12_4.cuda_cudart
          cudaPackages_12_4.cuda_nvrtc
          cudaPackages_12_4.libcublas
          cudaPackages_12_4.libcurand
        ];

        otherDeps = with pkgs; [
          zig
          toolchain
          pkg-config
          openssl
          openssl.dev
          pkgsStatic.oniguruma
          musl
          libiconv
        ];
        onigurumaStatic = pkgsStatic.oniguruma;
      in {
        _module.args = {inherit pkgs;};
        devShells = {
          default = (pkgs.mkShell.override {stdenv = pkgs.gccStdenv;}) {
            name = "fwj-env";
            buildInputs = otherDeps ++ defaultDeps ++ cudaDeps;
            packages = otherDeps ++ defaultDeps ++ cudaDeps;
            HF_HOME = "/shelf/hf_home";
            HF_TOKEN = builtins.getEnv "HF_TOKEN";

            # cuda old
            CUDA_PATH = "${pkgs.cudaPackages_12_4.cudatoolkit}";
            TORCH_USE_CUDA_DSA = "1";
            CUDA_VISIBLE_DEVICES = "0,1,2";
            TORCH_CUDA_ARCH_LIST = "8.9"; # support for 4090 not to compile useless compatibilities
            TRITON_LIBCUDA_PATH = "${nvidia-p2p}/lib/libcuda.so";

            NVCC_CCBIN = "${pkgs.gcc}/bin";

            # cuda new
            RUSTONIG_SYSTEM_LIBONIG = true;
            # CUDA_COMPUTE_CAP = "8.9";
            CUDA_TOOLKIT_ROOT_DIR = pkgs.lib.getDev pkgs.cudaPackages_12_4.cuda_cudart;

            NVCC_PREPEND_FLAGS = [
              "-I${pkgs.lib.getDev pkgs.cudaPackages_12_4.cuda_cudart}/include"
              "-I${pkgs.lib.getDev pkgs.cudaPackages_12_4.cudnn}/include"
              "-I${pkgs.lib.getDev pkgs.cudaPackages_12_4.cuda_cccl}/include"
              "-I${pkgs.lib.getDev nvidia-p2p}/include"
            ];
            NVCC_APPEND_FLAGS = [
              "-L${pkgs.cudaPackages_12_4.cuda_cudart.static}/lib"
              "-L${pkgs.cudaPackages_12_4.cudnn}/lib"
              "-L${pkgs.cudaPackages_12_4.nccl}/lib"
              "-L${pkgs.cudaPackages_12_4.nvidia_fs}/lib"
              "-L${pkgs.cudaPackages_12_4.cuda_cudart}/lib"
              "-L${pkgs.cudaPackages_12_4.cudatoolkit}/lib"
              "-L${nvidia-p2p}/lib"
              "-L${onigurumaStatic}/lib"
            ];

            PKG_CONFIG_PATH = pkgs.lib.makeSearchPath "lib/pkgconfig" (defaultDeps ++ cudaDeps ++ otherDeps);
            PKG_CONFIG_SYSROOT_DIR = "/";
            PKG_CONFIG = "${pkgs.pkg-config}/bin/pkg-config";
            RUSTFLAGS = [
              "-C target-feature=+crt-static"
              "-L${onigurumaStatic}/lib"
              "-L${pkgs.cudaPackages_12_4.cuda_cudart}/lib"
              "-L${nvidia-p2p}/lib"
              "-L/run/opengl-drivers/lib"
              "-L/run/opengl-drivers/lib64"
              "-L/run/opengl-drivers/lib/nvidia"
              "-L/run/opengl-drivers/lib64/nvidia"
            ];
            LD_LIBRARY_PATH = "/run/opengl-drivers/lib:${onigurumaStatic}/lib:${pkgs.cudaPackages_12_4.cuda_cudart}:${pkgs.lib.makeLibraryPath (cudaDeps ++ otherDeps)}/lib";

            shellHook = ''
              set -eu
              export TAILSCALE_IP=$(tailscale ip -4 2>/dev/null)
              export HF_TOKEN=${builtins.getEnv "HF_TOKEN"}
              export CUDA_PATH="${pkgs.cudaPackages_12_4.cudatoolkit}"
              export HF_HOME="/shelf/hf_home"
              export OMP_NUM_THREADS=32
            '';
            # :${pkgs.glibc}/lib"
          };
        };
      };
    };
}
