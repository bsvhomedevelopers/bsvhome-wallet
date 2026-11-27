// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

@MainActor
final class CollectiveWalletOnboardingUITests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testDedicatedIdentityReviewsCollectiveRulesWithoutSubmitting() throws {
        guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLLECTIVE_UI"] == "1" else {
            throw XCTSkip("Requires the explicitly provisioned isolated QA identity and read-only collective UI opt-in.")
        }
        guard ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == "A4BB51ED-D5E8-433D-8280-6CC2D8CE832F" else {
            XCTFail("Run this test only on the isolated phase-five QA simulator.")
            return
        }

        let app = XCUIApplication()
        app.launchArguments += ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()
        defer { app.terminate() }
        verifyCollectiveOverview(app, tab: "wowpay-tab-home")
        verifyCollectiveOverview(app, tab: "wowpay-tab-wallet")
        app.tapWalletControl("collectiveScan")
        XCTAssertTrue(app.staticTexts["Select a signing wallet with a BSV account."].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["wowpay-tab-wallet"].isSelected)
        XCTAssertTrue(element(app, identifier: "collectiveOverview").exists)
        XCTAssertTrue(element(app, identifier: "collectiveWalletList").exists)
        XCTAssertFalse(app.navigationBars["Scan"].exists)
        XCTAssertFalse(app.buttons["receiveAddress"].exists)

        app.tapWalletControl("walletBar")
        XCTAssertTrue(app.buttons["createLocalWallet"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["importLocalWallet"].exists)
        XCTAssertEqual(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "selectLocalWallet_")).count, 0)
        app.tapWalletControl("createLocalWallet")
        app.acceptWalletTermsIfNeeded()
        app.tapWalletControl("wowPayCreateMethodMultisig")

        let name = app.textFields["bsvMultisigCreateName"]
        XCTAssertTrue(name.waitForExistence(timeout: 30))
        XCTAssertTrue(app.staticTexts["Alignment QA Identity"].exists)
        XCTAssertTrue(app.staticTexts["Dedicated signing identity"].exists)
        XCTAssertFalse(app.buttons["bsvMultisigIdentityPicker"].exists)
        XCTAssertTrue(app.buttons["bsvMultisigCreateDedicatedIdentity"].exists)
        assertNoSecretPresentation(app)

        let walletName = "Alignment Safe Review"
        replaceText(in: name, with: walletName)
        replaceText(in: app.textFields["bsvMultisigCreateMember_1"], with: "QA signer")
        replaceText(in: app.textFields["bsvMultisigCreateMember_2"], with: "Reviewer two")
        replaceText(in: app.textFields["bsvMultisigCreateMember_3"], with: "Reviewer three")
        dismissKeyboard(app)
        tapFormButton(app, identifier: "bsvMultisigCreateAddMember", scrollingUp: true)
        replaceText(in: app.textFields["bsvMultisigCreateMember_4"], with: "Reviewer four")
        dismissKeyboard(app)
        tapFormButton(app, identifier: "bsvMultisigCreateThreshold", scrollingUp: false)
        app.tapWalletControl("bsvMultisigCreateThresholdOption_3")
        tapFormButton(app, identifier: "bsvMultisigCreateReview", scrollingUp: true)

        XCTAssertTrue(element(app, identifier: "bsvMultisigCreateReviewPage").waitForExistence(timeout: 30))
        XCTAssertTrue(app.staticTexts[walletName].exists)
        XCTAssertTrue(app.staticTexts["3/4"].exists)
        XCTAssertTrue(app.staticTexts["QA signer"].exists)
        XCTAssertTrue(app.staticTexts["Reviewer four"].exists)
        XCTAssertTrue(app.buttons["bsvMultisigCreateSubmit"].exists)
        assertNoSecretPresentation(app)

        try tapNativeBack(app)
        XCTAssertTrue(name.waitForExistence(timeout: 15))
        XCTAssertEqual(name.value as? String, walletName)
        XCTAssertEqual(app.textFields["bsvMultisigCreateMember_1"].value as? String, "QA signer")
        XCTAssertEqual(app.textFields["bsvMultisigCreateMember_4"].value as? String, "Reviewer four")
        XCTAssertTrue(app.buttons["bsvMultisigCreateThreshold"].label.contains("3 signatures / 4 members"))
        try tapNativeBack(app)
        XCTAssertTrue(app.buttons["wowPayCreateMethodMultisig"].waitForExistence(timeout: 15))
        app.tapWalletControl("Cancel")
        XCTAssertTrue(app.buttons["createLocalWallet"].waitForExistence(timeout: 15))
        XCTAssertEqual(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "selectLocalWallet_")).count, 0)
        try tapNativeBack(app)
        XCTAssertTrue(app.buttons["wowpay-tab-wallet"].isSelected)
        XCTAssertTrue(element(app, identifier: "collectiveWalletList").waitForExistence(timeout: 30))
        XCTAssertFalse(element(app, identifier: "collectiveNoIdentity").exists)
        verifyCollectiveOverview(app, tab: "wowpay-tab-home")

        let lateReview = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == true"), object: app.buttons["bsvMultisigCreateSubmit"])
        lateReview.isInverted = true
        wait(for: [lateReview], timeout: 10)
        XCTAssertFalse(app.staticTexts["Invite members"].exists)
        assertNoSecretPresentation(app)
    }

    private func verifyCollectiveOverview(_ app: XCUIApplication, tab: String) {
        app.tapWalletControl(tab)
        XCTAssertTrue(element(app, identifier: "collectiveOverview").waitForExistence(timeout: 30))
        XCTAssertTrue(element(app, identifier: "collectiveWalletList").waitForExistence(timeout: 30))
        for identifier in ["receive_button", "send_button", "scan", "wowPayAssetTab_overview", "wowPayAssetTab_address", "wowPayAssetTab_transactions", "wowPayAssetTab_about"] {
            XCTAssertFalse(app.buttons[identifier].exists)
        }
        XCTAssertFalse(app.buttons["Paymail"].exists)
        XCTAssertTrue(app.buttons["collectiveScan"].exists)
        XCTAssertFalse(app.buttons["wowPayWelcomeCreateWallet"].exists)
        assertNoSecretPresentation(app)
    }

    private func element(_ app: XCUIApplication, identifier: String) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

    private func replaceText(in field: XCUIElement, with value: String) {
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        let current = field.value as? String ?? ""
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count) + value)
        XCTAssertEqual(field.value as? String, value)
    }

    private func dismissKeyboard(_ app: XCUIApplication) {
        let key = app.keyboards.buttons["Return"]
        if key.exists {
            key.tap()
        }
    }

    private func tapFormButton(_ app: XCUIApplication, identifier: String, scrollingUp: Bool) {
        let button = app.buttons[identifier]
        let form = app.scrollViews["collectiveCreateRoot"]
        XCTAssertTrue(button.waitForExistence(timeout: 15))
        XCTAssertTrue(form.exists)
        for _ in 0 ..< 3 where !button.isHittable || !formTouchArea(app, form: form).contains(button.frame) {
            let touchArea = formTouchArea(app, form: form)
            if button.frame.minY < touchArea.minY {
                form.swipeDown()
            } else if button.frame.maxY > touchArea.maxY || scrollingUp {
                form.swipeUp()
            } else {
                form.swipeDown()
            }
        }
        XCTAssertTrue(formTouchArea(app, form: form).contains(button.frame), "The form button must be fully below the foreground navigation bar and inside the visible form.")
        XCTAssertTrue(button.isHittable)
        XCTAssertTrue(button.isEnabled)
        button.tap()
    }

    private func formTouchArea(_ app: XCUIApplication, form: XCUIElement) -> CGRect {
        let visibleForm = form.frame.intersection(app.frame)
        let navigationBottom = app.navigationBars.allElementsBoundByIndex.last {
            $0.frame.intersects(app.frame)
        }?.frame.maxY ?? visibleForm.minY
        let top = max(visibleForm.minY, navigationBottom)
        return CGRect(x: visibleForm.minX, y: top, width: visibleForm.width, height: max(0, visibleForm.maxY - top))
    }

    private func tapNativeBack(_ app: XCUIApplication) throws {
        let back = try XCTUnwrap(app.navigationBars.buttons.allElementsBoundByIndex.first { button in
            button.isHittable && !["Cancel", "Close"].contains(button.label)
        }, "A native back control is required; do not substitute the feature cancel action.")
        back.tap()
    }

    private func assertNoSecretPresentation(_ app: XCUIApplication) {
        XCTAssertFalse(app.staticTexts["word_0"].exists)
        XCTAssertFalse(app.buttons["verifyWalletBackup"].exists)
        XCTAssertFalse(app.buttons["continueWalletSecurityReminder"].exists)
    }
}
