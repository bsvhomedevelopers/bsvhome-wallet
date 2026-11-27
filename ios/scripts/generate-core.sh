#!/usr/bin/env bash
#
# Generate BsvHomeCore UniFFI sources and iOS Rust static libraries.
# Usage:
#   generate-core.sh [--force] [release]
#

set -euo pipefail

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
unset MACOSX_DEPLOYMENT_TARGET TVOS_DEPLOYMENT_TARGET WATCHOS_DEPLOYMENT_TARGET XROS_DEPLOYMENT_TARGET
unset SWIFT_DEBUG_INFORMATION_FORMAT SWIFT_DEBUG_INFORMATION_VERSION

CURRENT_DIR=$(dirname "$(realpath "$0")")
IOS_DIR="$CURRENT_DIR/.."
PROJ_PATH="$IOS_DIR/BsvHome.xcodeproj/project.pbxproj"
CORE_DIR="$IOS_DIR/../core"
STONE_DIR="$CORE_DIR/bsv_home_core"
PACKAGES_DIR="$IOS_DIR/Packages/BsvHomeCore"
SWIFT_BINDINGS="$PACKAGES_DIR/Sources/BsvHomeCore/BsvHomeCore.swift"
FFI_HEADER="$PACKAGES_DIR/Sources/BsvHomeCoreFFI/include/BsvHomeCoreFFI.h"

FORCE=0
POSITIONAL=()
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        *) POSITIONAL+=("$arg") ;;
    esac
done

BUILD_MODE="${POSITIONAL[0]:-${BUILD_MODE:-}}"
if [ -z "$BUILD_MODE" ] && [ "${CONFIGURATION:-Debug}" = "Release" ]; then
    BUILD_MODE="release"
fi

core_source_fingerprint() {
    {
        shasum -a 256 "$0"
        git -C "$CORE_DIR" rev-parse HEAD
        git -C "$CORE_DIR" diff
        git -C "$CORE_DIR" diff --cached
        git -C "$CORE_DIR" ls-files --others --exclude-standard -z | \
            (cd "$CORE_DIR" && xargs -0 stat -f '%m %z %N' 2>/dev/null || true)
    }
}

bsv_home_core_hash() {
    {
        core_source_fingerprint
        printf '%s\n' "$@"
    } | shasum -a 256 | cut -d' ' -f1
}

read_deployment_target() {
    /usr/libexec/PlistBuddy -c "Print" "$PROJ_PATH" | awk -F ' = ' '/IPHONEOS_DEPLOYMENT_TARGET/ { print $2; exit }'
}

generate_bindings() {
    local cache_dir="$IOS_DIR/build/.bsv-home-core-cache"
    local hash_file="$cache_dir/sources-${BUILD_MODE:-debug}.hash"
    local current_hash
    mkdir -p "$cache_dir"
    current_hash=$(bsv_home_core_hash "${BUILD_MODE:-debug}")

    if [ "$FORCE" -eq 0 ] && [ -f "$hash_file" ] && [ -f "$SWIFT_BINDINGS" ] && [ -f "$FFI_HEADER" ]; then
        if [ "$current_hash" = "$(cat "$hash_file")" ]; then
            echo "note: BsvHomeCore UniFFI sources unchanged (${BUILD_MODE:-debug}) - skipping regeneration"
            return 0
        fi
    fi

    if [ "$FORCE" -eq 1 ]; then
        echo "note: Forcing BsvHomeCore UniFFI source regeneration (${BUILD_MODE:-debug})..."
    else
        echo "note: BsvHomeCore sources changed - regenerating UniFFI sources (${BUILD_MODE:-debug})..."
    fi

    pushd "$STONE_DIR" > /dev/null
    env -u MACOSX_DEPLOYMENT_TARGET -u TVOS_DEPLOYMENT_TARGET -u WATCHOS_DEPLOYMENT_TARGET -u XROS_DEPLOYMENT_TARGET \
        -u SWIFT_DEBUG_INFORMATION_FORMAT -u SWIFT_DEBUG_INFORMATION_VERSION \
        BUILD_MODE=$BUILD_MODE IPHONEOS_DEPLOYMENT_TARGET=$(read_deployment_target) just bindgen-swift
    mkdir -p "$PACKAGES_DIR/Sources/BsvHomeCore" "$PACKAGES_DIR/Sources/BsvHomeCoreFFI/include"
    cp generated/swift/bsv_home_core.swift "$SWIFT_BINDINGS"
    cp generated/swift/BsvHomeCoreFFI.h "$FFI_HEADER"
    popd > /dev/null

    echo "$current_hash" > "$hash_file"
    echo "note: BsvHomeCore UniFFI sources regenerated successfully (${BUILD_MODE:-debug})"
}

build_ios_static_libraries() {
    local profile="debug"
    local build_flag=""
    local targets="${BSV_HOME_CORE_IOS_TARGETS:-aarch64-apple-ios-sim aarch64-apple-ios}"
    local host_arch
    host_arch="$(uname -m)"

    if [ "${BUILD_MODE:-}" = "release" ]; then
        profile="release"
        build_flag="--release"
    fi

    if [ "$host_arch" != "arm64" ]; then
        echo "error: Intel Macs are not supported for iOS BsvHomeCore builds." >&2
        exit 1
    fi

    echo "note: Building BsvHomeCore iOS static libraries ($profile)"
    for rust_target in $targets; do
        env -u MACOSX_DEPLOYMENT_TARGET -u TVOS_DEPLOYMENT_TARGET -u WATCHOS_DEPLOYMENT_TARGET -u XROS_DEPLOYMENT_TARGET \
            -u SWIFT_DEBUG_INFORMATION_FORMAT -u SWIFT_DEBUG_INFORMATION_VERSION \
            CARGO_PROFILE_RELEASE_LTO=fat \
            CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
            IPHONEOS_DEPLOYMENT_TARGET="$(read_deployment_target)" \
            cargo rustc --manifest-path "$STONE_DIR/Cargo.toml" --target "$rust_target" --lib ${build_flag} --crate-type staticlib
    done

    echo "note: BsvHomeCore iOS static libraries ready ($profile)"
}

generate_bindings
build_ios_static_libraries
