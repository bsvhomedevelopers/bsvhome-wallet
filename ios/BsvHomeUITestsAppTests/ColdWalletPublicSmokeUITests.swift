import Foundation
import XCTest

@MainActor
final class ColdWalletPublicSmokeUITests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testUnifiedSignerPublicDashboardPairingAndScannerCancellation() throws {
        let configuration = try configuration(for: .unifiedSigner)
        let app = launch(configuration)
        defer { app.terminate() }
        let copy = configuration.copy

        try signerDashboard(app, configuration)
        try homeHelpRoutes(app, configuration)
        try recordPublic("phase7-signer-dashboard", app: app, requiring: [walletHeader(app, configuration), app.buttons[copy.pair]])
        try tap(app.buttons[copy.pair])
        try pairing(app, configuration)
        try recordPublic("phase7-signer-public-pairing", app: app, requiring: [app.staticTexts[configuration.address].firstMatch, publicQr(app, copy)])
        try nativeBack(app, title: copy.pair)
        try signerDashboard(app, configuration)
        try tap(app.buttons[copy.pair])
        try pairing(app, configuration)
        try tapPairingDone(app, configuration)
        try signerDashboard(app, configuration)

        try tap(app.buttons[copy.scanRequest])
        try scanner(app, copy)
        if configuration.language == "en" {
            try tap(app.buttons[copy.photoLibrary].firstMatch)
            try dismissPhotoLibrary(app, copy)
            try scanner(app, copy)
            try require(app.buttons[copy.photoLibrary].firstMatch.waitForExistence(timeout: 15), "The Gallery cancellation must return to the existing scanner.")
            try require(app.buttons[copy.photoLibrary].firstMatch.isHittable, "The Gallery entry must become usable again on the original scanner.")
        }
        try tap(visibleCancel(app, label: copy.cancel))
        try signerDashboard(app, configuration)
        try tap(app.buttons["wowpay-tab-wallet"])
        try signerDashboard(app, configuration, home: false)
        try tap(app.buttons["wowpay-tab-home"])
        try signerDashboard(app, configuration)
    }

    func testUnifiedSignerInitialScanRequestBackgroundResumesSigningHomeAndReturnsToItsCaller() throws {
        let configuration = try configuration(for: .unifiedSigner)
        let app = launch(configuration)
        defer { app.terminate() }
        let copy = configuration.copy
        try signerDashboard(app, configuration)
        try tap(app.buttons["coldWalletDashboardSigning"])
        try scanner(app, copy)
        try require(app.state == .runningForeground, "The initial public scanner must be in the foreground before the lifecycle transition.")
        XCUIDevice.shared.press(.home)
        let background = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.state == .runningBackground || app.state == .runningBackgroundSuspended
        }, object: nil)
        try require(XCTWaiter.wait(for: [background], timeout: 15) == .completed, "The actual App must enter the background without being terminated.")
        app.activate()
        try require(app.wait(for: .runningForeground, timeout: 15), "The same App must return to the foreground.")
        try signingWorkspace(app, configuration)
        try nativeBack(app, title: copy.signing)
        try publicWorkspaceHome(app, configuration)
        try nativeBack(app, title: copy.title)
        try signerDashboard(app, configuration)
    }

    func testUnifiedSignerWalletDetailsSigningAndPairingReturnToTheRealDetails() throws {
        let configuration = try configuration(for: .unifiedSigner)
        let app = launch(configuration)
        defer { app.terminate() }
        let copy = configuration.copy
        try signerDashboard(app, configuration)
        try tap(walletHeader(app, configuration))
        try tap(app.buttons[copy.management].firstMatch)
        let settings = app.buttons.matching(NSPredicate(format: "label == %@ AND value == %@", copy.settings, configuration.walletName)).firstMatch
        try tap(settings)
        try coldWalletDetails(app, configuration)
        try tap(app.buttons["walletDetailColdSigning"])
        try signingWorkspace(app, configuration)
        try tap(app.buttons["coldWalletWorkspaceScanRequest"])
        try scanner(app, copy)
        try tap(visibleCancel(app, label: copy.cancel))
        try signingWorkspace(app, configuration)
        try nativeBack(app, title: copy.signing)
        try coldWalletDetails(app, configuration)
        try tap(app.buttons["walletDetailColdPairing"])
        try pairing(app, configuration)
        try nativeBack(app, title: copy.pair)
        try coldWalletDetails(app, configuration)
    }

    func testObserverPublicSendAndOrdinaryReceiveReturnToTheRealHome() throws {
        let configuration = try configuration(for: .observer)
        let app = launch(configuration)
        defer { app.terminate() }
        let copy = configuration.copy

        try observerMainHome(app, configuration)
        try tap(app.buttons["send_button"])
        if configuration.observerContext == .legacyProtocol {
            try observerRetainedSignedSend(app, copy)
            try recordPublic("phase8-observer-retained-signed-send", app: app, requiring: [app.navigationBars[copy.send], app.buttons[copy.pending]])
        } else {
            try observerSend(app, copy)
            try recordPublic("phase7-observer-empty-send", app: app, requiring: [app.navigationBars[copy.send], app.buttons[copy.createRequest]])
            try tap(app.buttons["coldWalletScanRecipient"])
            try scanner(app, copy)
            try tap(visibleCancel(app, label: copy.cancel))
            try observerSend(app, copy)
        }
        try nativeBack(app, title: copy.send)
        try observerMainHome(app, configuration)

        try tap(app.buttons["receive_button"])
        try ordinaryObserverReceive(app, configuration)
        try recordPublic("phase7-observer-public-receive", app: app, requiring: [app.navigationBars[copy.receivePayment], app.buttons["receiveAddress"]])
        try nativeBack(app, title: copy.receivePayment)
        try observerMainHome(app, configuration)
    }

    func testLegacyIsolatedPublicHomePairingAndScannerCancellation() throws {
        let configuration = try configuration(for: .legacyCold)
        let app = launch(configuration)
        defer { app.terminate() }
        let copy = configuration.copy

        try legacyHome(app, configuration)
        try recordPublic("phase7-legacy-isolated-home", app: app, requiring: [coldChoice(app, configuration.walletName), app.buttons[copy.pair]])
        try tap(app.buttons[copy.pair])
        try pairing(app, configuration)
        try recordPublic("phase7-legacy-public-pairing", app: app, requiring: [app.staticTexts[configuration.address].firstMatch, publicQr(app, copy)])
        try nativeBack(app, title: copy.pair)
        try legacyHome(app, configuration)
        try tap(app.buttons[copy.scanRequest])
        try scanner(app, copy)
        try require(!app.buttons[copy.photoLibrary].exists, "The isolated offline scanner must not offer its Gallery action.")
        let scannerButtons = app.navigationBars[copy.scan].buttons.allElementsBoundByIndex.filter(\.isHittable)
        try require(scannerButtons.count == 1 && scannerButtons[0].label == copy.cancel, "The isolated scanner toolbar must contain only Cancel.")
        try tap(visibleCancel(app, label: copy.cancel))
        try legacyHome(app, configuration)
        try tap(app.buttons[copy.pair])
        try pairing(app, configuration)
        try tap(app.buttons[copy.done])
        try legacyHome(app, configuration)
    }

    private func configuration(for role: Role) throws -> Configuration {
        let environment = ProcessInfo.processInfo.environment
        guard environment["BSVHOME_ALIGNMENT_COLD_UI"] == "1" else {
            throw XCTSkip("Requires explicit public-only phase-seven Cold UI opt-in and the already provisioned isolated device.")
        }
        try require(environment["SIMULATOR_UDID"]?.uppercased() == role.simulatorId, "Run this exact test only on its explicitly authorized base simulator with parallel testing disabled.")
        let language = environment["BSVHOME_ALIGNMENT_COLD_UI_LANGUAGE"] ?? "en"
        let copy = try Copy.forLanguage(language)
        guard let address = environment["BSVHOME_ALIGNMENT_COLD_UI_PUBLIC_ADDRESS"],
              (26 ... 35).contains(address.utf8.count), address.first == "1",
              address.unicodeScalars.allSatisfy({ CharacterSet.alphanumerics.contains($0) && $0.isASCII })
        else {
            throw Failure.publicAddressRequired
        }
        let contextName = environment["BSVHOME_ALIGNMENT_COLD_UI_OBSERVER_CONTEXT"] ?? ObserverContext.original.rawValue
        guard let observerContext = ObserverContext(rawValue: contextName) else { throw Failure.invalidObserverContext }
        if case .observer = role {
            try require(address == observerContext.address, "The explicit Observer context must use its own approved public address.")
        } else {
            try require(observerContext == .original, "An Observer context must not change another simulator role.")
        }
        return Configuration(role: role, language: language, address: address, copy: copy, observerContext: observerContext)
    }

    private func launch(_ configuration: Configuration) -> XCUIApplication {
        let app = XCUIApplication(bundleIdentifier: "com.bsvhome.ios")
        app.launchArguments += ["-AppleLanguages", "(\(configuration.language))", "-AppleLocale", configuration.copy.locale]
        app.launch()
        return app
    }

    private func signerDashboard(_ app: XCUIApplication, _ configuration: Configuration, home: Bool = true) throws {
        try present(walletHeader(app, configuration))
        try present(app.staticTexts[configuration.copy.signing].firstMatch)
        try present(app.buttons[configuration.copy.pair])
        try present(app.buttons[configuration.copy.scanRequest])
        try present(app.staticTexts[configuration.copy.disconnect].firstMatch)
        try require(!app.navigationBars[configuration.copy.title].exists, "Embedded Cold dashboard must use its real wallet header instead of an extra Cold navigation title.")
        if home {
            for action in ["scan", "receive_button", "send_button"] {
                try present(app.buttons[action])
                try require(app.buttons[action].isEnabled, "Cold Home keeps its visible ordinary actions available for the unsupported-help route.")
            }
        } else {
            try require(!app.buttons["scan"].exists && !app.buttons["send_button"].exists && !app.buttons["receive_button"].exists, "Cold Assets must not contain the Home quick-action grid.")
        }
        try require(!app.buttons["wowPayNewsSection"].exists, "The removed News module must not appear on Cold Home or Assets.")
        for tab in ["wowpay-tab-home", "wowpay-tab-wallet", "wowpay-tab-settings"] {
            try present(app.buttons[tab])
        }
        try noSecretPage(app, configuration.copy)
    }

    private func homeHelpRoutes(_ app: XCUIApplication, _ configuration: Configuration) throws {
        let copy = configuration.copy
        for action in ["coldWalletHeaderScan", "scan", "receive_button", "send_button"] {
            try tap(app.buttons[action])
            try unsupportedHelp(app, configuration)
            let bar = app.navigationBars.firstMatch
            try present(bar)
            guard let back = bar.buttons.allElementsBoundByIndex.first(where: \.isHittable) else { throw Failure.requiredControl }
            try tap(back)
            try signerDashboard(app, configuration)
        }
        try tap(app.buttons["coldWalletHeaderScan"])
        try unsupportedHelp(app, configuration)
        try tap(app.buttons["coldWalletOpenAssetsToSign"])
        try signerDashboard(app, configuration, home: false)
        try require(!app.staticTexts[copy.unsupported].exists, "The help footer must close its own route while opening Assets.")
        try tap(app.buttons["wowpay-tab-home"])
        try signerDashboard(app, configuration)
    }

    private func unsupportedHelp(_ app: XCUIApplication, _ configuration: Configuration) throws {
        let copy = configuration.copy
        try present(app.staticTexts[copy.unsupported].firstMatch)
        try present(app.staticTexts[copy.useObserver].firstMatch)
        try present(app.staticTexts[copy.signerRole].firstMatch)
        try present(app.buttons["coldWalletOpenAssetsToSign"])
        try noSecretPage(app, copy)
    }

    private func walletHeader(_ app: XCUIApplication, _ configuration: Configuration) -> XCUIElement {
        app.buttons.matching(identifier: "walletBar").matching(NSPredicate(format: "label CONTAINS %@", configuration.walletName)).firstMatch
    }

    private func coldWalletDetails(_ app: XCUIApplication, _ configuration: Configuration) throws {
        try noSecretPage(app, configuration.copy)
        try present(app.navigationBars[configuration.copy.wallet])
        let name = app.textFields["walletDetailName"]
        try present(name)
        try require(name.value as? String == configuration.walletName, "The actual wallet details must still belong to the provisioned public QA identity.")
        for _ in 0 ..< 2 where !app.buttons["walletDetailColdSigning"].isHittable {
            try noSecretPage(app, configuration.copy)
            try require(app.navigationBars[configuration.copy.wallet].exists, "Only the verified public wallet details may be scrolled to its cold actions.")
            app.scrollViews.firstMatch.swipeUp()
        }
        try present(app.buttons["walletDetailColdPairing"])
        try present(app.buttons["walletDetailColdSigning"])
    }

    private func signingWorkspace(_ app: XCUIApplication, _ configuration: Configuration) throws {
        let copy = configuration.copy
        let scan = app.buttons["coldWalletWorkspaceScanRequest"]
        try present(app.navigationBars[copy.signing])
        try present(coldChoice(app, configuration.walletName))
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            scan.exists && scan.isEnabled && scan.isHittable && !app.navigationBars[copy.scan].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [ready], timeout: 15) == .completed, "The real Signing workspace must be usable with no old scanner replay.")
        try noSecretPage(app, copy)
    }

    private func publicWorkspaceHome(_ app: XCUIApplication, _ configuration: Configuration) throws {
        let copy = configuration.copy
        let setup = app.staticTexts[copy.setupHint]
        try present(app.navigationBars[copy.title])
        try present(app.staticTexts[copy.choose].firstMatch)
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            setup.exists && setup.isHittable && !app.navigationBars[copy.signing].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [ready], timeout: 15) == .completed, "Signing Back must reveal public Cold Home: setupExists=\(setup.exists), setupHittable=\(setup.isHittable), signingBarExists=\(app.navigationBars[copy.signing].exists).")
        try require(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "coldWalletChoice-")).count == 0, "This exact-one-signer QA space must have no observer choices on Cold Home.")
        try require(!app.staticTexts[copy.chooseHint].exists, "An empty observer selector must show setup guidance instead of switch-wallet instructions.")
        try require(!app.buttons[copy.pending].exists && !app.buttons[copy.scanRequest].exists, "A selected signer must not expose observer Pending or Signing actions on Cold Home.")
        try noSecretPage(app, copy)
    }

    private func ordinaryObserverReceive(_ app: XCUIApplication, _ configuration: Configuration) throws {
        try present(app.navigationBars[configuration.copy.receivePayment])
        try present(app.buttons["receiveAddress"])
        let addressLabel = app.buttons["receiveAddress"].label.replacingOccurrences(of: "\u{200B}", with: "")
        try require(addressLabel.contains(configuration.address), "The actual public Receive control must show the provisioned address.")
        try require(!app.buttons["coldWalletOpenAssetsToSign"].exists, "An observer uses the actual public Receive page, not the signer help route.")
        try noSecretPage(app, configuration.copy)
    }

    private func observerMainHome(_ app: XCUIApplication, _ configuration: Configuration) throws {
        try present(walletHeader(app, configuration))
        try present(app.buttons["send_button"])
        try present(app.buttons["receive_button"])
        try require(app.buttons["send_button"].isEnabled && app.buttons["receive_button"].isEnabled, "The provisioned xPub observer must expose unsigned Send and public Receive.")
        try noSecretPage(app, configuration.copy)
    }

    private func observerRetainedSignedSend(_ app: XCUIApplication, _ copy: Copy) throws {
        try present(app.navigationBars[copy.send])
        try present(app.buttons[copy.pending])
        try require(!app.textFields[copy.address].exists && !app.textViews[copy.address].exists, "A retained unbroadcast payment must hide the new recipient input.")
        try require(!app.textFields[copy.amount].exists && !app.textViews[copy.amount].exists, "A retained unbroadcast payment must hide the new amount input.")
        try require(!app.buttons[copy.createRequest].exists && !app.buttons["coldWalletScanRecipient"].exists, "The retained payment must block creating or scanning a new request.")
        try require(!app.buttons[copy.cancel].exists, "An already signed payment must not expose unsigned-request cancellation.")
        try noSecretPage(app, copy)
    }

    private func observerSend(_ app: XCUIApplication, _ copy: Copy) throws {
        try present(app.navigationBars[copy.send])
        try present(app.textFields[copy.address])
        try present(app.textFields[copy.amount])
        try present(app.buttons[copy.createRequest])
        try require(!app.buttons[copy.createRequest].isEnabled, "The empty form must not prepare a payment.")
        try noSecretPage(app, copy)
    }

    private func legacyHome(_ app: XCUIApplication, _ configuration: Configuration) throws {
        try present(app.navigationBars[configuration.copy.title])
        try present(coldChoice(app, configuration.walletName))
        try present(app.buttons[configuration.copy.pair])
        try present(app.buttons[configuration.copy.scanRequest])
        try present(app.staticTexts[configuration.copy.disconnect].firstMatch)
        try require(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "coldWalletChoice-")).count == 1, "The isolated legacy space must contain exactly its provisioned wallet.")
        try require(!app.buttons["wowpay-tab-home"].exists && !app.buttons["walletBar"].exists, "The legacy cold space must remain isolated from the daily wallet root.")
        try noSecretPage(app, configuration.copy)
    }

    private func tapPairingDone(_ app: XCUIApplication, _ configuration: Configuration) throws {
        let done = app.buttons[configuration.copy.done]
        for _ in 0 ..< 2 where !done.isHittable {
            try noSecretPage(app, configuration.copy)
            try require(app.navigationBars[configuration.copy.pair].exists && app.staticTexts[configuration.address].exists && publicQr(app, configuration.copy).exists, "Only the verified public Pairing page may be scrolled to its Done action.")
            app.scrollViews.firstMatch.swipeUp()
        }
        try tap(done)
    }

    private func pairing(_ app: XCUIApplication, _ configuration: Configuration) throws {
        try present(app.navigationBars[configuration.copy.pair])
        try present(app.staticTexts[configuration.copy.pairingHint].firstMatch)
        try present(app.staticTexts[configuration.walletName].firstMatch)
        try present(app.staticTexts[configuration.address].firstMatch)
        try present(publicQr(app, configuration.copy))
        try present(app.buttons[configuration.copy.done])
        try present(app.staticTexts["coldWalletQrFrameCount"])
        try require(!app.buttons[configuration.copy.copy].exists, "Pairing uses the actual public QR and Done, without an extra Copy action.")
        try noSecretPage(app, configuration.copy)
    }

    private func scanner(_ app: XCUIApplication, _ copy: Copy) throws {
        try present(app.navigationBars[copy.scan])
        try present(app.buttons[copy.cancel].firstMatch)
        try noSecretPage(app, copy)
    }

    private func coldChoice(_ app: XCUIApplication, _ name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS %@", "coldWalletChoice-", name)).firstMatch
    }

    private func publicQr(_ app: XCUIApplication, _ copy: Copy) -> XCUIElement {
        app.images.matching(NSPredicate(format: "label BEGINSWITH %@", copy.qrPrefix)).firstMatch
    }

    private func dismissPhotoLibrary(_ app: XCUIApplication, _ copy: Copy) throws {
        let gallery = app.buttons[copy.photoLibrary].firstMatch
        let scannerCancel = app.navigationBars[copy.scan].buttons[copy.cancel].firstMatch
        let cancelButtons = app.buttons.matching(NSPredicate(format: "identifier == %@ OR label == %@", "Cancel", "Cancel"))
        let closeButtons = app.buttons.matching(NSPredicate(format: "identifier == %@ OR label == %@", "Close", "Close"))
        let covered = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            !gallery.isHittable && !scannerCancel.isHittable
        }, object: nil)
        let coverage = XCTContext.runActivity(named: "Wait for Gallery to cover the original scanner") { _ in
            XCTWaiter.wait(for: [covered], timeout: 15)
        }
        try require(coverage == .completed, "Gallery must cover both its original entry and the scanner Cancel control.")

        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            !gallery.isHittable && !scannerCancel.isHittable
                && (closeButtons.allElementsBoundByAccessibilityElement.contains(where: { $0.exists && $0.isHittable }) || cancelButtons.allElementsBoundByAccessibilityElement.contains(where: { $0.exists && $0.isHittable }))
        }, object: nil)
        let readiness = XCTContext.runActivity(named: "Wait for a foreground Gallery Cancel or Close control") { _ in
            XCTWaiter.wait(for: [ready], timeout: 15)
        }
        try require(readiness == .completed && !gallery.isHittable && !scannerCancel.isHittable, "Only a foreground Gallery dismissal control may be used.")
        guard let dismissal = closeButtons.allElementsBoundByAccessibilityElement.first(where: { $0.exists && $0.isHittable }) ?? cancelButtons.allElementsBoundByAccessibilityElement.first(where: { $0.exists && $0.isHittable }) else {
            throw Failure.requiredControl
        }
        try tap(dismissal)

        let restored = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            gallery.isHittable && scannerCancel.isHittable
        }, object: nil)
        let restoration = XCTContext.runActivity(named: "Wait for the same scanner and Gallery entry to return") { _ in
            XCTWaiter.wait(for: [restored], timeout: 15)
        }
        try require(restoration == .completed, "Closing Gallery must restore the original scanner, not close its sheet.")
    }

    private func visibleCancel(_ app: XCUIApplication, label: String) throws -> XCUIElement {
        try present(app.buttons[label].firstMatch)
        let buttons = app.buttons.matching(NSPredicate(format: "identifier == %@ OR label == %@", label, label))
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            buttons.allElementsBoundByAccessibilityElement.contains(where: { $0.exists && $0.isHittable })
        }, object: nil)
        try require(XCTWaiter.wait(for: [ready], timeout: 15) == .completed, "The scanner cancellation control must be visibly usable.")
        guard let button = buttons.allElementsBoundByAccessibilityElement.first(where: { $0.exists && $0.isHittable }) else {
            throw Failure.requiredControl
        }
        return button
    }

    private func nativeBack(_ app: XCUIApplication, title: String) throws {
        let bar = app.navigationBars[title]
        try present(bar)
        guard let back = bar.buttons.allElementsBoundByIndex.first(where: \.isHittable) else { throw Failure.requiredControl }
        back.tap()
    }

    private func noSecretPage(_ app: XCUIApplication, _ copy: Copy) throws {
        try require(!app.secureTextFields.firstMatch.exists, "A secret input or authorization prompt is outside this public-only smoke.")
        for identifier in ["word_0", "verifyWalletBackup", "continueWalletSecurityReminder", "continueWalletBackup", "transferPasswordCurrent", "transferPasswordNew", "transferPasswordConfirmation", "transferPasswordLoading", "transferPasswordNotLoaded", "transferPasswordSubmit", "transferPasswordCancel"] {
            try require(!app.descendants(matching: .any).matching(identifier: identifier).firstMatch.exists, "A secret flow is outside this public-only smoke.")
        }
        try require(!app.switches[copy.backupConfirmation].exists, "A backup page is outside this public-only smoke.")
    }

    private func recordPublic(_ name: String, app: XCUIApplication, requiring controls: [XCUIElement]) throws {
        for control in controls {
            try present(control)
        }
        try require(!app.secureTextFields.firstMatch.exists, "Never capture a secret input or authorization prompt.")
        try require(!app.staticTexts["word_0"].exists && !app.buttons["verifyWalletBackup"].exists, "Never capture a secret phrase or backup flow.")
        recordScreen(name, app: app)
    }

    private func tap(_ element: XCUIElement) throws {
        try present(element)
        try require(element.isEnabled && element.isHittable, "Only an enabled visible public control may be tapped.")
        element.tap()
    }

    private func present(_ element: XCUIElement) throws {
        try require(element.waitForExistence(timeout: 30), "The expected public page or control is unavailable; no screenshot was requested.")
    }

    private func require(_ condition: Bool, _ message: String) throws {
        guard condition else {
            XCTFail(message)
            throw Failure.unexpectedPublicState
        }
    }

    private enum Failure: Error {
        case publicAddressRequired
        case invalidObserverContext
        case unsupportedLanguage
        case requiredControl
        case unexpectedPublicState
    }

    private enum Role {
        case unifiedSigner, observer, legacyCold

        var simulatorId: String {
            switch self {
            case .unifiedSigner: "F47186EF-B7A9-4956-A2A2-C04663361205"
            case .observer: "4A910C7A-C540-40C9-9405-04F46F5E81C1"
            case .legacyCold: "1DE87C77-6CB7-4FB7-BB69-30242C8FEEF0"
            }
        }

        var walletName: String {
            switch self {
            case .unifiedSigner: "Alignment Cold Signer QA"
            case .observer: "Alignment Cold Observer QA"
            case .legacyCold: "Alignment Legacy Cold QA"
            }
        }
    }

    private enum ObserverContext: String, Equatable {
        case original
        case legacyProtocol

        var walletName: String {
            switch self {
            case .original: Role.observer.walletName
            case .legacyProtocol: "Alignment Legacy Observer Protocol"
            }
        }

        var address: String {
            switch self {
            case .original: "1NT3xN86b4iHLZrNrBLSqrT91iFSCCSKPy"
            case .legacyProtocol: "1yu4FKnAicZNZQotBknfaxRPAjdk95kKL"
            }
        }
    }

    private struct Configuration {
        let role: Role
        let language: String
        let address: String
        let copy: Copy
        let observerContext: ObserverContext

        var walletName: String {
            if case .observer = role {
                return observerContext.walletName
            }
            return role.walletName
        }
    }

    private struct Copy {
        let locale: String
        let title: String
        let scan: String
        let management: String
        let settings: String
        let wallet: String
        let receivePayment: String
        let pair: String
        let signing: String
        let choose: String
        let chooseHint: String
        let setupHint: String
        let pending: String
        let unsupported: String
        let useObserver: String
        let signerRole: String
        let scanRequest: String
        let observer: String
        let send: String
        let receive: String
        let address: String
        let amount: String
        let createRequest: String
        let cancel: String
        let done: String
        let copy: String
        let pairingHint: String
        let receiveHint: String
        let disconnect: String
        let activity: String
        let qrPrefix: String
        let photoLibrary: String
        let backupConfirmation: String

        static func forLanguage(_ language: String) throws -> Copy {
            switch language {
            case "en":
                Copy(locale: "en_US",
                     title: "BSV Cold Wallet",
                     scan: "Scan",
                     management: "Wallet management",
                     settings: "Settings",
                     wallet: "Wallet",
                     receivePayment: "Receive Payment",
                     pair: "Connect an observer wallet",
                     signing: "Offline signing",
                     choose: "Choose a wallet",
                     chooseHint: "Tap a card to switch. The actions below use the selected wallet.",
                     setupHint: "Connect a cold wallet from Import Wallet first.",
                     pending: "Continue Last Transaction",
                     unsupported: "Cold wallets do not support this action.",
                     useObserver: "Use the online watch wallet to send and receive payments.",
                     signerRole: "This device's cold wallet stores keys and signs transactions offline.",
                     scanRequest: "Scan Signing Request",
                     observer: "Open Observer Wallet",
                     send: "Send",
                     receive: "Receive",
                     address: "Address",
                     amount: "Amount",
                     createRequest: "Create Signing Request",
                     cancel: "Cancel",
                     done: "Done",
                     copy: "Copy",
                     pairingHint: "Scan this pairing QR on the online device. Compare the first receive address on both devices.",
                     receiveHint: "This is a normal BSV mainnet address. Ordinary wallets can send BSV here.",
                     disconnect: "Keep this dedicated device disconnected.",
                     activity: "Activity",
                     qrPrefix: "Frame ",
                     photoLibrary: "Select from Photo Library",
                     backupConfirmation: "I have written down and checked my secret phrase.")
            case "zh-Hans":
                Copy(locale: "zh_CN",
                     title: "BSV 冷钱包",
                     scan: "扫码",
                     management: "钱包管理",
                     settings: "设置",
                     wallet: "钱包",
                     receivePayment: "收款",
                     pair: "连接观察钱包",
                     signing: "离线签名",
                     choose: "选择钱包",
                     chooseHint: "点击卡片切换，下方操作使用当前选中的钱包。",
                     setupHint: "请先从导入钱包入口连接冷钱包。",
                     pending: "继续上一笔交易",
                     unsupported: "冷钱包不支持此操作",
                     useObserver: "收付款请在在线观察钱包操作",
                     signerRole: "本机冷钱包负责保管密钥和离线签名。",
                     scanRequest: "扫描待签名交易",
                     observer: "查看观察钱包",
                     send: "发送",
                     receive: "接收",
                     address: "地址",
                     amount: "发送金额",
                     createRequest: "生成待签名交易",
                     cancel: "取消",
                     done: "完成",
                     copy: "复制",
                     pairingHint: "使用在线设备扫描配对二维码，并核对两台设备的首个收款地址是否一致。",
                     receiveHint: "这是普通 BSV 主网地址，普通钱包可以直接向此地址转入 BSV。",
                     disconnect: "请让这台专用设备保持断网。",
                     activity: "交易记录",
                     qrPrefix: "第 ",
                     photoLibrary: "从照片中选取",
                     backupConfirmation: "我已抄写并核对助记词。")
            case "ar":
                Copy(locale: "ar_SA",
                     title: "محفظة BSV باردة",
                     scan: "مسح",
                     management: "Wallet management",
                     settings: "إعدادات",
                     wallet: "محفظة",
                     receivePayment: "استلام الدفع",
                     pair: "ربط محفظة مراقبة",
                     signing: "توقيع دون اتصال",
                     choose: "اختر محفظة",
                     chooseHint: "اضغط على بطاقة للتبديل. تستخدم الإجراءات أدناه المحفظة المحددة.",
                     setupHint: "اربط محفظة باردة من استيراد المحفظة أولاً.",
                     pending: "متابعة آخر معاملة",
                     unsupported: "المحافظ الباردة لا تدعم هذا الإجراء.",
                     useObserver: "استخدم محفظة المراقبة المتصلة لإرسال المدفوعات واستلامها.",
                     signerRole: "تحفظ المحفظة الباردة على هذا الجهاز المفاتيح وتوقّع المعاملات دون اتصال.",
                     scanRequest: "مسح طلب التوقيع",
                     observer: "فتح محفظة المراقبة",
                     send: "يرسل",
                     receive: "يستلم",
                     address: "عنوان",
                     amount: "Amount",
                     createRequest: "إنشاء طلب توقيع",
                     cancel: "يلغي",
                     done: "تم",
                     copy: "نسخ",
                     pairingHint: "امسح هذا الرمز بالجهاز المتصل. قارن أول عنوان استقبال على الجهازين.",
                     receiveHint: "هذا عنوان عادي على شبكة BSV الرئيسية. يمكن للمحافظ العادية إرسال BSV إليه.",
                     disconnect: "أبقِ هذا الجهاز المخصص غير متصل.",
                     activity: "نشاط",
                     qrPrefix: "الإطار ",
                     photoLibrary: "اختر من مكتبة الصور",
                     backupConfirmation: "دوّنت عبارتي السرية وتحققت منها.")
            default: throw Failure.unsupportedLanguage
            }
        }
    }
}
