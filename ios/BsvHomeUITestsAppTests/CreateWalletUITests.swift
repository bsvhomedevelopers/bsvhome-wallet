// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

@MainActor
final class CreateWalletUITests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testWelcomeOnlineCreationStopsBeforeSecretAndRelaunches() {
        let app = XCUIApplication()
        app.launch()
        defer { app.terminate() }
        app.verifyWalletWelcome()
        recordScreen("01-welcome", app: app)
        app.openWalletAction("createLocalWallet")
        if app.buttons["acceptWalletTerms"].waitForExistence(timeout: 2) {
            recordScreen("02-terms", app: app)
        }
        app.acceptWalletTermsIfNeeded()
        app.verifyWalletCreationMethods()
        recordScreen("03-create-method", app: app)
        app.tapWalletControl("wowPayCreateMethodOnline")
        app.verifyWalletSecurityReminder()

        app.terminate()
        app.launch()
        app.verifyWalletWelcome()
        app.tapWalletControl("wowPayWelcomeSkip")
        app.acceptWalletTermsIfNeeded()
        app.verifyWalletCreationMethods()
        app.tapWalletControl("wowPayCreateMethodOnline")
        app.verifyWalletSecurityReminder()
    }

    func testCreateBSVWalletNavigateThreeTabsAndReceive() throws {
        throw XCTSkip("Full create, backup and receive UI acceptance requires a verified way to prevent recovery phrases from being retained in XCTest screenshots, action logs and failure diagnostics.")
    }
}
