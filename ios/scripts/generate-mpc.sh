#!/usr/bin/env bash
set -euo pipefail

IOS_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MPC_ROOT="$IOS_ROOT/Packages/WowPayMpc/Native"
MPC_MODE="${1:-debug}"
MPC_DEPLOYMENT_TARGET="$(/usr/libexec/PlistBuddy -c Print "$IOS_ROOT/BsvHome.xcodeproj/project.pbxproj" | awk -F ' = ' '!found && /IPHONEOS_DEPLOYMENT_TARGET/ { print $2; found = 1 }')"
MPC_TARGETS="${WOWPAY_MPC_IOS_TARGETS:-aarch64-apple-ios-sim aarch64-apple-ios}"
MPC_BUILD_FLAGS=(build --manifest-path "$MPC_ROOT/Cargo.toml" --locked)

case "$MPC_MODE" in
    debug) ;;
    release) MPC_BUILD_FLAGS+=(--release) ;;
    *) exit 2 ;;
esac

for MPC_TARGET in $MPC_TARGETS; do
    env -u MACOSX_DEPLOYMENT_TARGET -u TVOS_DEPLOYMENT_TARGET -u WATCHOS_DEPLOYMENT_TARGET -u XROS_DEPLOYMENT_TARGET \
        IPHONEOS_DEPLOYMENT_TARGET="$MPC_DEPLOYMENT_TARGET" cargo "${MPC_BUILD_FLAGS[@]}" --target "$MPC_TARGET"
done
