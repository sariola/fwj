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

# Create a new GitHub release or update an existing one
create-release VERSION:
    @echo "Creating or updating release v{{VERSION}}..."
    gh release view v{{VERSION}} > /dev/null 2>&1 && \
    (echo "Release v{{VERSION}} already exists. Updating..." && \
     gh release edit v{{VERSION}} --notes "$(gh release view v{{VERSION}} --json body -q .body)") || \
    (echo "Checking if tag v{{VERSION}} exists..." && \
     (git ls-remote --exit-code --tags origin v{{VERSION}} > /dev/null 2>&1 && \
      (echo "Tag v{{VERSION}} exists but no release. Creating release..." && \
       gh release create v{{VERSION}} --generate-notes) || \
      (echo "Creating new tag and release v{{VERSION}}..." && \
       git tag v{{VERSION}} && \
       git push origin v{{VERSION}} && \
       gh release create v{{VERSION}} --generate-notes)))
    @echo "Release created or updated. Waiting for 5 seconds..."
    sleep 5

# Compress binaries
compress-binaries VERSION:
    @echo "Compressing binaries..."
    mkdir -p target/release-packages
    tar -czf target/release-packages/fwj-{{VERSION}}-x86_64-unknown-linux-musl.tar.gz -C target/x86_64-unknown-linux-musl/release fwj
    tar -czf target/release-packages/fwj-{{VERSION}}-aarch64-unknown-linux-musl.tar.gz -C target/aarch64-unknown-linux-musl/release fwj
    tar -czf target/release-packages/fwj-{{VERSION}}-x86_64-apple-darwin.tar.gz -C target/x86_64-apple-darwin/release fwj
    tar -czf target/release-packages/fwj-{{VERSION}}-universal2-apple-darwin.tar.gz -C target/universal2-apple-darwin/release fwj

# Upload built binaries to the GitHub release
upload-release-assets VERSION:
    @echo "Checking for compressed binary files..."
    # Check if all files exist before uploading
    [ -f "target/release-packages/fwj-{{VERSION}}-x86_64-unknown-linux-musl.tar.gz" ] || \
    (echo "Error: x86_64-linux compressed binary not found" && exit 1)

    [ -f "target/release-packages/fwj-{{VERSION}}-aarch64-unknown-linux-musl.tar.gz" ] || \
    (echo "Error: aarch64-linux compressed binary not found" && exit 1)

    [ -f "target/release-packages/fwj-{{VERSION}}-x86_64-apple-darwin.tar.gz" ] || \
    (echo "Error: x86_64-macos compressed binary not found" && exit 1)

    [ -f "target/release-packages/fwj-{{VERSION}}-universal2-apple-darwin.tar.gz" ] || \
    (echo "Error: universal2-macos compressed binary not found" && exit 1)

    @echo "All compressed binary files found."

    # Check if release already has assets
    gh release view v{{VERSION}} --json assets -q '.assets[].name' | grep -q ".tar.gz" && \
    (echo "Release v{{VERSION}} already has assets. Do you want to overwrite them? (y/N)" && \
     read -r response && \
     if [ "$response" != "y" ] && [ "$response" != "Y" ]; then \
         echo "Aborting upload process." && exit 1; \
     fi) || true

    @echo "Proceeding with upload..."

    # If all files exist and user confirmed (if necessary), proceed with upload
    gh release upload v{{VERSION}} \
        --clobber \
        target/release-packages/fwj-{{VERSION}}-x86_64-unknown-linux-musl.tar.gz \
        target/release-packages/fwj-{{VERSION}}-aarch64-unknown-linux-musl.tar.gz \
        target/release-packages/fwj-{{VERSION}}-x86_64-apple-darwin.tar.gz \
        target/release-packages/fwj-{{VERSION}}-universal2-apple-darwin.tar.gz || \
    (echo "Error uploading assets. Checking release status..." && \
     gh release view v{{VERSION}} && \
     exit 1)

    @echo "Asset upload completed successfully."

# Perform a full release process
release VERSION:
    @echo "Starting release process for version {{VERSION}}..."
    just build-release-static
    just build-release-static-macos
    just build-release-static-macos-universal2
    just compress-binaries {{VERSION}}
    just create-release {{VERSION}}
    just upload-release-assets {{VERSION}}
    @echo "Release process completed."

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
