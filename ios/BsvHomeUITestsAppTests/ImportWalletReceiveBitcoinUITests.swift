// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

@MainActor
final class ImportWalletReceiveBitcoinUITests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testImportViewOnlyBSVWalletsSelectionRestartAndDeletion() {
        let app = XCUIApplication()
        app.launchArguments += ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()
        defer { app.terminate() }
        guard app.buttons["wowPayWelcomeImportWallet"].waitForExistence(timeout: 15) else {
            XCTFail("Run this lifecycle test only on a fresh isolated simulator with no existing wallets.")
            return
        }
        app.verifyWalletWelcome()
        let firstName = "Alignment Watch A"
        let secondName = "Alignment Watch B"
        importWatchWallet(in: app, address: "1BitcoinEaterAddressDontSendf59kuE", name: firstName)
        verifyPersonalBsv(in: app, address: "1BitcoinEaterAddressDontSendf59kuE")
        if ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_PUBLIC_QR_UI"] == "1" {
            guard ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == "B88052A0-BA3D-4CF4-AD46-F3A835DD5B38" else {
                XCTFail("The public payment QR fixture requires its isolated public-only regression simulator.")
                return
            }
            verifyPublicFundingPhotoRoute(in: app, walletName: firstName)
        }

        app.tapWalletControl("walletBar")
        let localWallets = localWallets(in: app)
        XCTAssertTrue(localWallets.firstMatch.waitForExistence(timeout: 15))
        guard localWallets.count == 1 else {
            XCTFail("The isolated lifecycle test must contain only its newly imported watch-only wallet.")
            return
        }
        let firstIdentifier = localWallets.firstMatch.identifier
        localWallets.firstMatch.tap()
        verifySelectedWallet(in: app, name: firstName)
        importWatchWallet(in: app, address: "1CounterpartyXXXXXXXXXXXXXXXUWLpVr", name: secondName)
        app.tapWalletControl("walletBar")
        let secondWallet = localWallets.matching(NSPredicate(format: "label CONTAINS %@", secondName)).firstMatch
        XCTAssertTrue(secondWallet.waitForExistence(timeout: 15))
        let secondIdentifier = secondWallet.identifier
        XCTAssertNotEqual(firstIdentifier, secondIdentifier)
        let identifiers: Set<String> = [firstIdentifier, secondIdentifier]
        guard verifyWalletIdentifiers(in: app, expected: identifiers) else { return }
        app.tapWalletControl(firstIdentifier)
        verifySelectedWallet(in: app, name: firstName)

        relaunch(app, expecting: firstName)
        app.tapWalletControl("walletBar")
        guard verifyWalletIdentifiers(in: app, expected: identifiers) else { return }
        app.tapWalletControl(secondIdentifier)
        verifySelectedWallet(in: app, name: secondName)
        relaunch(app, expecting: secondName)

        app.tapWalletControl("walletBar")
        guard verifyWalletIdentifiers(in: app, expected: identifiers) else { return }
        deleteWatchWallet(in: app, name: secondName, checkCancellation: true)
        let remainingSettings = walletSettings(in: app, name: firstName)
        XCTAssertTrue(remainingSettings.waitForExistence(timeout: 15))
        XCTAssertTrue(walletSettings(in: app, name: secondName).waitForNonExistence(timeout: 10))
        relaunch(app, expecting: firstName)

        app.tapWalletControl("walletBar")
        guard verifyWalletIdentifiers(in: app, expected: [firstIdentifier]) else { return }
        deleteWatchWallet(in: app, name: firstName, checkCancellation: false)
        app.verifyWalletWelcome()
        app.terminate()
        app.launch()
        app.verifyWalletWelcome()
    }

    func testPublicGalleryWithTheKnownInterruptedWatchFixture() throws {
        guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_PUBLIC_QR_RESUME"] == "1" else {
            throw XCTSkip("Requires the known interrupted public-only Gallery fixture.")
        }
        guard ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == "B88052A0-BA3D-4CF4-AD46-F3A835DD5B38" else {
            XCTFail("Run the public Gallery fixture only on its isolated regression simulator.")
            return
        }
        let app = XCUIApplication()
        app.launchArguments += ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()
        defer { app.terminate() }
        let name = "Alignment Watch A"
        let identifier = "selectLocalWallet_view_bitcoinsv_1BitcoinEaterAddressDontSendf59kuE"
        verifySelectedWallet(in: app, name: name)
        app.tapWalletControl("walletBar")
        guard verifyWalletIdentifiers(in: app, expected: [identifier]) else { return }
        XCTAssertTrue(app.buttons[identifier].label.contains(name))
        app.tapWalletControl(identifier)
        verifyPublicFundingPhotoRoute(in: app, walletName: name)
        app.tapWalletControl("walletBar")
        guard verifyWalletIdentifiers(in: app, expected: [identifier]) else { return }
        deleteWatchWallet(in: app, name: name, checkCancellation: false)
        app.verifyWalletWelcome()
        app.terminate()
        app.launch()
        app.verifyWalletWelcome()
    }

    private func verifyPublicFundingPhotoRoute(in app: XCUIApplication, walletName: String) {
        app.tapWalletControl("scan")
        XCTAssertTrue(app.navigationBars["Scan"].waitForExistence(timeout: 15))
        app.tapWalletControl("Select from Photo Library")
        let photo = app.images.matching(NSPredicate(format: "label BEGINSWITH %@", "Photo,")).firstMatch
        guard photo.waitForExistence(timeout: 15) else {
            XCTFail("The isolated photo library must contain the prepared public payment QR.\n" + app.debugDescription)
            return
        }
        XCTAssertTrue(app.frame.contains(photo.frame))
        XCTAssertGreaterThan(photo.frame.width, 0)
        XCTAssertGreaterThan(photo.frame.height, 0)
        photo.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        let unavailable = app.staticTexts["No spendable personal BSV wallet. Create or import one first."]
        XCTAssertTrue(unavailable.waitForExistence(timeout: 30))
        XCTAssertTrue(app.navigationBars["Scan"].waitForNonExistence(timeout: 15))
        XCTAssertFalse(app.buttons["bsvPublicFundingPrepare"].exists)
        XCTAssertFalse(app.buttons["bsvPublicFundingSubmit"].exists)
        recordScreen("phase6-01-public-funding-no-payer", app: app)
        app.tapWalletControl("bsvPublicFundingOpenRecovery")
        XCTAssertTrue(app.buttons["bsvPublicFundingOpenRecovery"].waitForNonExistence(timeout: 15))
        XCTAssertTrue(unavailable.waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["bsvPublicFundingPrepare"].exists)
        recordScreen("phase6-02-public-funding-recovery-no-payer", app: app)
        app.tapWalletControl("Cancel")
        verifySelectedWallet(in: app, name: walletName)
        XCTAssertTrue(app.buttons["wowpay-tab-home"].isSelected)
    }

    private func importWatchWallet(in app: XCUIApplication, address: String, name: String) {
        app.openWalletAction("importLocalWallet")
        app.acceptWalletTermsIfNeeded()
        app.tapWalletControl("wowPayImportMethodWatch")
        let input = app.descendants(matching: .any).matching(identifier: "wowPayImportInputField").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 10))
        XCTAssertFalse(input.isEnabled)
        app.tapWalletControl("wowPayImportSelectNetwork")
        let bsvNetwork = app.buttons.containing(.staticText, identifier: "Bitcoin SV").firstMatch
        XCTAssertTrue(bsvNetwork.waitForExistence(timeout: 10))
        bsvNetwork.tap()
        XCTAssertTrue(input.waitForExistence(timeout: 10))
        XCTAssertTrue(input.isEnabled)
        input.tap()
        input.typeText(address)
        let submit = app.buttons["wowPayImportInputSubmit"]
        if !submit.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(submit.isEnabled)
        submit.tap()
        XCTAssertTrue(app.buttons["finishWalletSetup"].waitForExistence(timeout: 15))
        XCTAssertEqual(app.textFields.count, 1)
        let nameField = app.textFields.firstMatch
        guard let currentName = nameField.value as? String else {
            XCTFail("The real wallet setup name field is unavailable.")
            return
        }
        nameField.tap()
        nameField.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: currentName.count) + name)
        XCTAssertEqual(nameField.value as? String, name)
        app.tapWalletControl("finishWalletSetup")
        app.verifyPhaseOneTabs()
        verifySelectedWallet(in: app, name: name)
    }

    private func verifySelectedWallet(in app: XCUIApplication, name: String) {
        let header = app.buttons.matching(identifier: "walletBar").matching(NSPredicate(format: "label CONTAINS %@", name)).firstMatch
        XCTAssertTrue(header.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["receive_button"].exists)
        XCTAssertTrue(app.buttons["receive_button"].isEnabled)
        XCTAssertFalse(app.buttons["send_button"].isEnabled)
        XCTAssertFalse(app.buttons["wowPayWelcomeImportWallet"].exists)
    }

    private func verifyPersonalBsv(in app: XCUIApplication, address: String) {
        XCTAssertTrue(app.buttons["Total assets (USD)"].exists)
        recordScreen("phase3-01-home", app: app)
        app.tapWalletControl("receive_button")
        let receiveAddress = app.buttons["receiveAddress"]
        XCTAssertTrue(receiveAddress.waitForExistence(timeout: 15))
        XCTAssertTrue(receiveAddress.label.replacingOccurrences(of: "\u{200B}", with: "").contains(address))
        let share = app.buttons["receiveShare"]
        XCTAssertTrue(share.waitForExistence(timeout: 15))
        XCTAssertTrue(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true"), object: share)], timeout: 15) == .completed)
        XCTAssertTrue(app.buttons["receiveCopy"].isEnabled)
        recordScreen("phase3-02-receive", app: app)
        share.tap()
        let shareSheet = app.otherElements["ActivityListView"].firstMatch
        XCTAssertTrue(shareSheet.waitForExistence(timeout: 10))
        recordScreen("phase3-02a-share", app: app)
        let closeShare = shareSheet.buttons["header.closeButton"]
        XCTAssertTrue(closeShare.waitForExistence(timeout: 10))
        closeShare.tap()
        XCTAssertTrue(shareSheet.waitForNonExistence(timeout: 10))
        XCTAssertTrue(app.buttons["receiveCopy"].isHittable)
        app.tapWalletControl("receiveCopy")
        app.navigationBars.buttons.firstMatch.tap()
        XCTAssertTrue(app.buttons["receive_button"].waitForExistence(timeout: 10))

        app.tapWalletControl("wowpay-tab-wallet")
        recordScreen("phase3-02b-overview", app: app)
        for identifier in ["overview", "address", "transactions", "about"] {
            XCTAssertTrue(app.buttons["wowPayAssetTab_" + identifier].waitForExistence(timeout: 15))
        }
        app.tapWalletControl("wowPayAssetTab_address")
        let assetAddress = app.staticTexts["wowPayAssetAddress"]
        XCTAssertTrue(assetAddress.waitForExistence(timeout: 15))
        XCTAssertEqual(assetAddress.label.replacingOccurrences(of: "\u{200B}", with: ""), address)
        XCTAssertFalse(app.buttons["wowPayBsvAddAddress"].exists)
        recordScreen("phase3-03-address", app: app)
        app.tapWalletControl("wowPayAssetTab_transactions")
        recordScreen("phase3-04-transactions", app: app)
        app.tapWalletControl("wowPayAssetTab_about")
        XCTAssertTrue(app.staticTexts["About BSV"].waitForExistence(timeout: 10))
        recordScreen("phase3-05-about", app: app)
        app.tapWalletControl("wowPayAssetTab_overview")
        app.tapWalletControl("wowpay-tab-settings")
        app.tapWalletControl("Address management")
        let profileAddress = app.buttons["receiveAddress"]
        XCTAssertTrue(profileAddress.waitForExistence(timeout: 15))
        XCTAssertTrue(profileAddress.label.replacingOccurrences(of: "\u{200B}", with: "").contains(address))
        recordScreen("phase3-06-profile-receive", app: app)
        app.tapWalletControl("Close")
        app.tapWalletControl("Security")
        app.tapWalletControl("manageOnlineTransferPassword")
        XCTAssertTrue(app.staticTexts["Watch-only wallet. You don't control these funds."].waitForExistence(timeout: 15))
        XCTAssertEqual(app.secureTextFields.count, 0)
        XCTAssertFalse(app.buttons["transferPasswordSubmit"].exists)
        recordScreen("phase4-01-watch-password-blocked", app: app)
        app.tapWalletControl("transferPasswordCancel")
        XCTAssertTrue(app.buttons["manageOnlineTransferPassword"].waitForExistence(timeout: 10))
        app.navigationBars.buttons.firstMatch.tap()
        app.tapWalletControl("wowpay-tab-home")
        recordScreen("phase3-07-home-settled", app: app)
    }

    private func relaunch(_ app: XCUIApplication, expecting name: String) {
        app.terminate()
        app.launch()
        app.verifyPhaseOneTabs()
        verifySelectedWallet(in: app, name: name)
    }

    private func localWallets(in app: XCUIApplication) -> XCUIElementQuery {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "selectLocalWallet_"))
    }

    private func verifyWalletIdentifiers(in app: XCUIApplication, expected: Set<String>) -> Bool {
        let wallets = localWallets(in: app)
        XCTAssertTrue(wallets.firstMatch.waitForExistence(timeout: 15))
        guard Set(wallets.allElementsBoundByIndex.map(\.identifier)) == expected else {
            XCTFail("Refusing to continue because the local wallet IDs do not exactly match this test's imported wallets.")
            return false
        }
        return true
    }

    private func walletSettings(in app: XCUIApplication, name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label == %@ AND value == %@", "Settings", name)).firstMatch
    }

    private func deleteWatchWallet(in app: XCUIApplication, name: String, checkCancellation: Bool) {
        let management = app.buttons["Wallet management"].firstMatch
        XCTAssertTrue(management.waitForExistence(timeout: 10))
        management.tap()
        let settings = walletSettings(in: app, name: name)
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        let delete = app.buttons["Delete"].firstMatch
        XCTAssertTrue(delete.waitForExistence(timeout: 10))
        delete.tap()
        let confirmation = app.alerts.firstMatch
        XCTAssertTrue(confirmation.waitForExistence(timeout: 10))
        if checkCancellation {
            confirmation.buttons["Cancel"].tap()
            XCTAssertTrue(confirmation.waitForNonExistence(timeout: 5))
            XCTAssertTrue(delete.exists)
            delete.tap()
            XCTAssertTrue(confirmation.waitForExistence(timeout: 10))
        }
        confirmation.buttons["Delete"].tap()
    }
}
