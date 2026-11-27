// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

extension XCUIApplication {
    var isOnboarding: Bool {
        buttons["wowPayWelcomeCreateWallet"].exists || buttons["wowPayWelcomeImportWallet"].exists || buttons["createLocalWallet"].exists
    }

    func chooseDailyDevicePurposeIfNeeded() {
        let dailyPurpose = buttons["selectDailyDevicePurpose"]
        if dailyPurpose.waitForExistence(timeout: 2) {
            dailyPurpose.tap()
        }
    }

    func tapContinue() {
        if buttons["continueWalletSecurityReminder"].exists {
            tapWalletControl("continueWalletSecurityReminder")
        } else {
            buttons["Continue"].firstMatch.tap()
        }
    }

    func getWords() -> [String] {
        (0 ..< 12).map { staticTexts["word_\($0)"].label }
    }

    func tapWalletBar() {
        tapWalletControl("wowpay-tab-settings")
        tapWalletControl("openWalletManager")
    }

    func openWalletAction(_ identifier: String) {
        let action = buttons[identifier]
        let welcomeIdentifier: String? = switch identifier {
        case "createLocalWallet": "wowPayWelcomeCreateWallet"
        case "importLocalWallet": "wowPayWelcomeImportWallet"
        default: nil
        }
        let identifiers = [identifier, "wowpay-tab-settings"] + [welcomeIdentifier].compactMap(\.self)
        let ready = buttons.matching(NSPredicate(format: "identifier IN %@", identifiers)).firstMatch
        XCTAssertTrue(ready.waitForExistence(timeout: 15))
        if let welcomeIdentifier, buttons[welcomeIdentifier].exists {
            tapWalletControl(welcomeIdentifier)
            return
        }
        if !action.exists {
            tapWalletBar()
        }
        tapWalletControl(identifier)
    }

    func acceptWalletTermsIfNeeded() {
        let accept = buttons["acceptWalletTerms"]
        let ready = buttons.matching(NSPredicate(format: "identifier IN %@", ["acceptWalletTerms", "wowPayCreateMethodOnline", "wowPayImportMethodPhrase"])).firstMatch
        XCTAssertTrue(ready.waitForExistence(timeout: 15))
        if accept.exists {
            let terms = switches.allElementsBoundByIndex
            XCTAssertEqual(terms.count, 3)
            for term in terms where term.value as? String != "1" {
                term.tap()
            }
            XCTAssertTrue(accept.isEnabled)
            accept.tap()
        }
    }

    func verifyWalletWelcome() {
        XCTAssertTrue(buttons["wowPayWelcomeCreateWallet"].waitForExistence(timeout: 15))
        XCTAssertTrue(buttons["wowPayWelcomeImportWallet"].exists)
        XCTAssertTrue(buttons["wowPayWelcomeSkip"].exists)
        XCTAssertFalse(buttons["wowpay-tab-home"].exists)
        XCTAssertFalse(buttons["selectDailyDevicePurpose"].exists)
    }

    func verifyWalletCreationMethods() {
        for identifier in ["wowPayCreateMethodOnline", "wowPayCreateMethodCold", "wowPayCreateMethodMultisig"] {
            XCTAssertTrue(buttons[identifier].waitForExistence(timeout: 15), "Missing creation method \(identifier)")
        }
        XCTAssertTrue(buttons["wowPayCreateMethodOnline"].isEnabled)
    }

    func verifyWalletSecurityReminder() {
        let continueButton = buttons["continueWalletSecurityReminder"]
        XCTAssertTrue(continueButton.waitForExistence(timeout: 15))
        XCTAssertTrue(continueButton.isEnabled)
        XCTAssertFalse(staticTexts["word_0"].exists)
    }

    func tapWalletControl(_ identifier: String, timeout: TimeInterval = 15) {
        let button = buttons[identifier].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: timeout), "Missing control \(identifier)")
        button.tap()
    }

    func verifyPhaseOneTabs() {
        for identifier in ["wowpay-tab-wallet", "wowpay-tab-settings", "wowpay-tab-home"] {
            tapWalletControl(identifier)
        }
        XCTAssertFalse(buttons["wowpay-tab-swap"].exists)
    }
}
