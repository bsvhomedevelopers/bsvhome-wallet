#!/usr/bin/env bash
set -euo pipefail

COMMIT="${1:?Usage: just upgrade-test <commit-or-tag>}"
REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKTREE_DIR="/tmp/bsvhome-upgrade-test-$$"
SIMULATOR_NAME="${SIMULATOR_NAME:-iPhone 17}"
SIMULATOR_DEST="platform=iOS Simulator,name=$SIMULATOR_NAME"
OLD_DERIVED_DATA="$WORKTREE_DIR/ios/build/DerivedData"

cleanup() {
    echo "==> Cleaning up worktree"
    cd "$REPO_ROOT"
    git worktree remove --force "$WORKTREE_DIR" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> Phase 1: Build old version ($COMMIT)"

git worktree add "$WORKTREE_DIR" "$COMMIT"
cd "$WORKTREE_DIR"
git submodule update --init -- ios/Submodules
cd ios

OLD_PROJECT="BsvHome.xcodeproj"
OLD_UI_SCHEME="BsvHomeUITests"
OLD_GENERATE_RECIPE="generate-core"
if [ ! -d "$OLD_PROJECT" ]; then
    OLD_PROJECT="Gem.xcodeproj"
    OLD_UI_SCHEME="GemUITests"
    OLD_GENERATE_RECIPE="generate-stone"
fi

echo "==> Generating Core (old)"
just "$OLD_GENERATE_RECIPE"

echo "==> Building old version for UI testing"
set -o pipefail && xcodebuild -project "$OLD_PROJECT" \
    -scheme "$OLD_UI_SCHEME" \
    -testPlan ui_tests \
    ONLY_ACTIVE_ARCH=YES \
    -destination "$SIMULATOR_DEST" \
    -derivedDataPath "$OLD_DERIVED_DATA" \
    -allowProvisioningUpdates \
    -allowProvisioningDeviceRegistration \
    build-for-testing | xcbeautify --quieter --is-ci

echo "==> Resetting simulator"
cd "$REPO_ROOT"
just ios reset-simulator

echo "==> Running ImportWalletReceiveBitcoinUITests (old version)"
cd "$WORKTREE_DIR/ios"
set -o pipefail && xcodebuild -project "$OLD_PROJECT" \
    -scheme "$OLD_UI_SCHEME" \
    -testPlan ui_tests \
    ONLY_ACTIVE_ARCH=YES \
    -destination "$SIMULATOR_DEST" \
    -derivedDataPath "$OLD_DERIVED_DATA" \
    -allowProvisioningUpdates \
    -allowProvisioningDeviceRegistration \
    -only-testing "$OLD_UI_SCHEME/ImportWalletReceiveBitcoinUITests" \
    test-without-building | xcbeautify --quieter --is-ci

echo "==> Phase 2: Build current version"
cd "$REPO_ROOT/ios"

just build-for-testing-integration

DERIVED_DATA="build/DerivedData"

echo "==> Running UpgradeVerificationTests (current version)"
set -o pipefail && xcodebuild -project BsvHome.xcodeproj \
    -scheme BsvHomeUITests \
    -testPlan ui_tests \
    ONLY_ACTIVE_ARCH=YES \
    -destination "$SIMULATOR_DEST" \
    -derivedDataPath "$DERIVED_DATA" \
    -allowProvisioningUpdates \
    -allowProvisioningDeviceRegistration \
    -only-testing BsvHomeUITests/UpgradeVerificationTests \
    test-without-building | xcbeautify --quieter --is-ci

echo "==> Upgrade test passed!"
