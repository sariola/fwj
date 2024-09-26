# Build release binaries for Linux (x86_64 and aarch64)
build-release-static:
    RUSTFLAGS="-C target-feature=+crt-static" cargo zigbuild \
    --target x86_64-unknown-linux-musl --release
    RUSTFLAGS="-C target-feature=+crt-static" cargo zigbuild \
    --target aarch64-unknown-linux-musl --release

# Build release binary for macOS (x86_64)
build-release-static-macos:
    docker run --rm -it -v $(pwd):/io -w /io -e RUSTFLAGS="-C target-feature=+crt-static" messense/cargo-zigbuild \
    cargo zigbuild --release --target x86_64-apple-darwin
    sudo chown -R $USER:$(id -gn $USER) target/x86_64-apple-darwin

# Build release binary for macOS (universal2)
build-release-static-macos-universal2:
    docker run --rm -it -v $(pwd):/io -e RUSTFLAGS="-C target-feature=+crt-static" -w /io messense/cargo-zigbuild \
    cargo zigbuild --release --target universal2-apple-darwin
    sudo chown -R $USER:$(id -gn $USER) target/universal2-apple-darwin
    sudo chown -R $USER:$(id -gn $USER) target/aarch64-apple-darwin

# Create a new GitHub release
create-release VERSION:
    gh release create v{{VERSION}} --generate-notes

# Upload built binaries to the GitHub release
upload-release-assets VERSION:
    gh release upload v{{VERSION}} \
        target/x86_64-unknown-linux-musl/release/fwj \
        target/aarch64-unknown-linux-musl/release/fwj \
        target/x86_64-apple-darwin/release/fwj \
        target/universal2-apple-darwin/release/fwj

    gh release upload v{{VERSION}} \
        --clobber \
        target/x86_64-unknown-linux-musl/release/fwj#fwj-x86_64-unknown-linux-musl \
        target/aarch64-unknown-linux-musl/release/fwj#fwj-aarch64-unknown-linux-musl \
        target/x86_64-apple-darwin/release/fwj#fwj-x86_64-apple-darwin \
        target/universal2-apple-darwin/release/fwj#fwj-universal2-apple-darwin

# Perform a full release process
release VERSION:
    just build-release-static
    just build-release-static-macos
    just build-release-static-macos-universal2
    just create-release {{VERSION}}
    just upload-release-assets {{VERSION}}

# Release process instructions
@release-instructions:
    echo "To create a new release, follow these steps:"
    echo "1. Update the version in Cargo.toml"
    echo "2. Commit the changes: git commit -am 'Bump version to X.Y.Z'"
    echo "3. Create a new git tag: git tag vX.Y.Z"
    echo "4. Push the changes and tag: git push && git push --tags"
    echo "5. Run the release command: just release X.Y.Z"
    echo "   (Replace X.Y.Z with the new version number)"
    echo "6. Verify the release on GitHub: https://github.com/sariola/fwj/releases"

# Default command to show instructions
default:
    @just --list
    @echo "\nFor release instructions, run: just release-instructions"
