import Foundation
import XCTest

@MainActor
final class SettingsPublicMatrixUITests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testPublicProfileSettingsPreferencesAndSecurityLabels() throws {
        let configuration = try configuration()
        let app = XCUIApplication(bundleIdentifier: "com.bsvhome.ios")
        app.launchArguments = ["-AppleLanguages", "(\(configuration.language.rawValue))", "-AppleLocale", configuration.copy.locale]
        app.launch()
        defer { app.terminate() }
        try publicPage(app)
        try tap(app.buttons["wowpay-tab-home"], app: app)
        let header = app.buttons["walletBar"].firstMatch
        try present(header)
        guard let selectedWalletName = configuration.role.walletNames.first(where: { header.label.contains($0) }) else {
            throw Failure.publicRoute
        }
        if configuration.variant == .compact || configuration.role == .compactPublic {
            try require(app.frame.width <= 375, "Compact acceptance requires an actual compact viewport; a label or window scaling is insufficient.")
        }
        try capture("home-top", app: app, configuration: configuration, anchor: header)
        try tap(app.buttons["wowpay-tab-wallet"], app: app)
        try capture("assets-top", app: app, configuration: configuration, anchor: header)
        try tap(app.buttons["wowpay-tab-home"], app: app)
        try require(!app.buttons["wowPayNewsSection"].exists, "The removed News module must not appear on Home.")
        try tap(app.buttons["wowpay-tab-settings"], app: app)
        try capture("profile-top", app: app, configuration: configuration, anchor: app.buttons["openWalletManager"])
        try present(app.buttons["settingsWallets"])
        try present(app.buttons["settingsSecurity"])
        try reveal(app.buttons["settingsPreferences"], app: app)
        try require(!app.buttons["openProfileSettings"].exists, "Settings actions must open directly from the profile without an intermediate Settings page.")
        try require(app.buttons["wowpay-tab-settings"].isHittable, "Wallets, Security and Preferences must belong to the profile root.")
        try require(!app.buttons["settingsDeveloper"].exists, "Developer must remain off in the existing default QA state; this test never changes the flag.")
        try capture("profile-settings", app: app, configuration: configuration, anchor: app.buttons["settingsPreferences"])
        try tap(app.buttons["settingsPreferences"], app: app)
        try reveal(app.buttons["settingsPreferencesNetworks"], app: app)
        try capture("preferences", app: app, configuration: configuration, anchor: app.buttons["settingsPreferencesNetworks"])
        try tap(app.buttons["settingsPreferencesNetworks"], app: app)
        try present(app.buttons["settingsNetwork_bitcoinsv"])
        try require(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "settingsNetwork_")).count == 1, "The product network chooser must expose only its BSV network.")
        try capture("networks", app: app, configuration: configuration, anchor: app.buttons["settingsNetwork_bitcoinsv"])
        if ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_STAGE8_KEYBOARD"] == "1" {
            try verifyNetworkKeyboardAndEmptyFilter(app, configuration: configuration)
        }
        try tap(app.buttons["settingsNetwork_bitcoinsv"], app: app)
        let chainDetail = app.descendants(matching: .any).matching(identifier: "settingsChainDetail").firstMatch
        try present(chainDetail)
        let loaded = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in !app.descendants(matching: .any).matching(identifier: "chainSettingsLoading").firstMatch.exists }, object: nil)
        try require(XCTWaiter.wait(for: [loaded], timeout: 40) == .completed, "The public network snapshot must finish or expose its existing error state.")
        try capture("bsv-network", app: app, configuration: configuration, anchor: chainDetail)
        try nativeBack(app, title: "Bitcoin SV", returningTo: app.buttons["settingsServiceStatus"])
        try tap(app.buttons["settingsServiceStatus"], app: app)
        let status = app.descendants(matching: .any).matching(identifier: "settingsServiceStatusRoot").firstMatch
        try present(status)
        try present(app.staticTexts["WhatsOnChain"].firstMatch)
        try present(app.staticTexts["DefiLlama"].firstMatch)
        try capture("service-status", app: app, configuration: configuration, anchor: status)
        try nativeBack(app, title: configuration.copy.status, returningTo: app.buttons["settingsNetwork_bitcoinsv"])
        try nativeBack(app, title: configuration.copy.networks, returningTo: app.buttons["settingsPreferencesContacts"])
        try tap(app.buttons["settingsPreferencesContacts"], app: app)
        let contacts = app.descendants(matching: .any).matching(identifier: "settingsContactsRoot").firstMatch
        try capture("contacts", app: app, configuration: configuration, anchor: contacts)
        try nativeBack(app, title: configuration.copy.contacts, returningTo: app.buttons["settingsPreferencesNetworks"])
        try nativeBack(app, title: configuration.copy.preferences, returningTo: app.buttons["settingsSecurity"])
        try tap(app.buttons["settingsSecurity"], app: app)
        let online = app.buttons["manageOnlineTransferPassword"]
        let offline = app.buttons["manageOfflineSigningPassword"]
        try present(online)
        try reveal(offline, app: app)
        try require(online.label.precomposedStringWithCanonicalMapping == configuration.copy.onlinePassword.precomposedStringWithCanonicalMapping, "The online password action must show the actual locale's online label.")
        try require(offline.label.precomposedStringWithCanonicalMapping == configuration.copy.offlinePassword.precomposedStringWithCanonicalMapping, "The offline signing password action must show its distinct actual localized label.")
        try capture("security-labels-only", app: app, configuration: configuration, anchor: offline)
        if ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_STAGE8_OFFLINE_TAB_EXIT"] == "1" {
            try verifyOfflineManagementTabExit(app, configuration: configuration)
        }
        try nativeBack(app, title: configuration.copy.security, returningTo: app.buttons["settingsPreferences"])
        try require(app.buttons["wowpay-tab-settings"].isHittable, "Native back from Security must return directly to the profile root.")
        try tap(app.buttons["wowpay-tab-home"], app: app)
        try present(header)
        try require(header.label.contains(selectedWalletName), "Read-only settings navigation must preserve the exact selected QA wallet.")
        try publicPage(app)
    }

    private func verifyOfflineManagementTabExit(_ app: XCUIApplication, configuration: Configuration) throws {
        try require(configuration.role == .unifiedSigner && configuration.language == .english, "Offline management consumer acceptance is limited to the explicitly authorized F471 signer in English.")
        try publicPage(app)
        let online = app.buttons["manageOnlineTransferPassword"]
        let offline = app.buttons["manageOfflineSigningPassword"]
        try present(online)
        try reveal(offline, app: app)
        try require(offline.isEnabled, "Only the explicit Offline Signing Password row may enter the local read-only status workflow.")
        offline.tap()
        let submit = app.buttons["transferPasswordSubmit"]
        let cancel = app.buttons["transferPasswordCancel"]
        let loading = app.descendants(matching: .any).matching(identifier: "transferPasswordLoading").firstMatch
        let notLoaded = app.descendants(matching: .any).matching(identifier: "transferPasswordNotLoaded").firstMatch
        let loadError = app.descendants(matching: .any).matching(identifier: "transferPasswordLoadError").firstMatch
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.state == .runningForeground && submit.exists && cancel.exists && !loading.exists && !notLoaded.exists && !loadError.exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [ready], timeout: 30) == .completed, "The actual offline backend must complete its local status read before the external asset route is opened.")
        guard let url = URL(string: "bsvhome://tokens/bitcoinsv") else { throw Failure.invalidConfiguration }
        XCUIDevice.shared.system.open(url)
        let overview = app.buttons["wowPayAssetTab_overview"]
        let address = app.buttons["wowPayAssetTab_address"]
        let assetMenu = app.buttons["wowPayAssetMore"]
        let assetNavigation = app.navigationBars["BSV"].firstMatch
        let arrived = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.state == .runningForeground && assetNavigation.exists && assetNavigation.isHittable && overview.exists && overview.isHittable && address.exists && assetMenu.exists
                && !submit.exists && !cancel.exists && !loading.exists && !notLoaded.exists && !loadError.exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [arrived], timeout: 30) == .completed, "The public asset URL must reach the registered BSV asset detail and retire the actual password modal; a selected tab alone is insufficient.")
        try publicPage(app)
        try verifyColdAssetDetailSubpages(app, configuration: configuration)
        try tap(app.buttons["wowpay-tab-settings"], app: app)
        try present(online)
        try present(offline)
        try require(app.navigationBars[configuration.copy.security].firstMatch.isHittable, "Returning directly to Settings must retain the original Security path.")
        try require(online.label == configuration.copy.onlinePassword && offline.label == configuration.copy.offlinePassword, "Both explicit password actions must remain on the retained public Security page.")
        try publicPage(app)
    }

    private func verifyColdAssetDetailSubpages(_ app: XCUIApplication, configuration: Configuration) throws {
        try publicPage(app)
        let addressTab = app.buttons["wowPayAssetTab_address"]
        try tap(addressTab, app: app)
        let address = app.descendants(matching: .any).matching(identifier: "wowPayAssetAddress").firstMatch
        let addressContent = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            addressTab.exists && addressTab.isSelected && address.exists && address.isHittable
                && app.staticTexts["My BSV address"].exists && app.staticTexts["Address history"].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [addressContent], timeout: 30) == .completed, "The actual selected Address tab must render its public address and address-history content without invoking any address action.")
        let transactionsTab = app.buttons["wowPayAssetTab_transactions"]
        try tap(transactionsTab, app: app)
        let emptyHistory = app.staticTexts["Your activity will appear here"].firstMatch
        let transactionsContent = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            transactionsTab.exists && transactionsTab.isSelected && app.staticTexts["Activity"].exists
                && emptyHistory.exists && emptyHistory.isHittable
        }, object: nil)
        try require(XCTWaiter.wait(for: [transactionsContent], timeout: 40) == .completed, "The actual zero-balance QA history must finish loading and render its successful empty-history state; an error, spinner or merely selected tab is insufficient.")
        let aboutTab = app.buttons["wowPayAssetTab_about"]
        try tap(aboutTab, app: app)
        let aboutContent = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            aboutTab.exists && aboutTab.isSelected && app.staticTexts["About BSV"].exists
                && app.staticTexts["Bitcoin SV native asset for low-cost global payments."].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [aboutContent], timeout: 30) == .completed, "The actual About tab must render its existing BSV title and public description.")
        let overviewTab = app.buttons["wowPayAssetTab_overview"]
        try tap(overviewTab, app: app)
        let overviewContent = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            overviewTab.exists && overviewTab.isSelected && app.buttons["wowPayAllTransactions"].exists
                && !app.staticTexts["About BSV"].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [overviewContent], timeout: 30) == .completed, "Returning to Overview must restore its actual recent-transactions section without activating it or any payment control.")
        let signing = app.buttons["coldWalletDashboardSigning"]
        try nativeBack(app, title: "BSV", returningTo: signing)
        let header = app.buttons["walletBar"].firstMatch
        let dashboard = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.buttons["wowpay-tab-wallet"].exists && app.buttons["wowpay-tab-wallet"].isSelected
                && signing.exists && signing.isHittable && app.buttons["coldWalletDashboardPairing"].exists
                && header.exists && header.isHittable
                && configuration.role.walletNames.contains(where: { header.label.contains($0) })
                && !app.buttons["wowPayAssetMore"].exists && !overviewTab.exists
                && !app.buttons["scan"].exists && !app.buttons["send_button"].exists && !app.buttons["receive_button"].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [dashboard], timeout: 30) == .completed, "Native Back must remove the pushed asset page and restore the original cold Assets dashboard for the same QA wallet.")
        try publicPage(app)
    }

    private func verifyNetworkKeyboardAndEmptyFilter(_ app: XCUIApplication, configuration: Configuration) throws {
        try publicPage(app)
        try present(app.buttons["settingsNetwork_bitcoinsv"])
        try present(app.buttons["settingsServiceStatus"])
        let navigation = app.navigationBars[configuration.copy.networks].firstMatch
        try require(navigation.isHittable, "Only the actual Networks page may receive the fixed public filter text.")
        let fields = app.searchFields.allElementsBoundByIndex.filter(\.isHittable)
        try require(fields.count == 1, "The actual Networks page must expose exactly one usable native search field.")
        let field = fields[0]
        try requireEmptySearch(field)
        try tap(field, app: app)
        let keyboard = app.keyboards.firstMatch
        try require(keyboard.waitForExistence(timeout: 10) && keyboard.frame.width > 0 && keyboard.frame.height > 0 && keyboard.keys.firstMatch.exists, "This keyboard row requires an actual visible software keyboard with real keys; no system setting is changed to manufacture it.")
        let query = "__phase8_no_chain__"
        field.typeText(query)
        let empty = app.descendants(matching: .any).matching(identifier: "settingsNetworksEmptyState").firstMatch
        let filtered = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            !app.buttons["settingsNetwork_bitcoinsv"].exists && empty.exists && keyboard.exists && keyboard.keys.firstMatch.exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [filtered], timeout: 15) == .completed, "The fixed public filter must remove BSV and render the actual empty state while the keyboard remains visible.")
        try require(field.value as? String == query, "Only the exact test-owned public characters may be cleared.")
        try present(app.staticTexts[configuration.copy.emptyNetworks].firstMatch)
        try capture("networks-keyboard-empty-filter", app: app, configuration: configuration, anchor: empty)
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: query.count))
        let restored = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.buttons["settingsNetwork_bitcoinsv"].exists && !empty.exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [restored], timeout: 15) == .completed, "Deleting only the known public query must restore the original BSV row.")
        try requireEmptySearch(field)
        let list = app.descendants(matching: .any).matching(identifier: "settingsNetworksList").firstMatch
        try present(list)
        try publicPage(app)
        let start = list.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.2))
        let finish = list.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85))
        start.press(forDuration: 0.05, thenDragTo: finish)
        let dismissed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in !app.keyboards.firstMatch.exists }, object: nil)
        try require(XCTWaiter.wait(for: [dismissed], timeout: 10) == .completed, "The existing interactive scroll-dismiss behavior must actually dismiss the software keyboard; no guessed Cancel control is tapped.")
        try present(app.buttons["settingsNetwork_bitcoinsv"])
        try requireEmptySearch(field)
        try capture("networks-filter-restored-keyboard-dismissed", app: app, configuration: configuration, anchor: app.buttons["settingsNetwork_bitcoinsv"])
        try require(configuration.language == .english, "Native search-close acceptance uses the observed English control on the two explicitly English keyboard rows.")
        let close = navigation.buttons.matching(NSPredicate(format: "label == %@", "Close")).allElementsBoundByIndex.filter(\.isHittable)
        try require(close.count == 1, "The active native search must expose its uniquely observed Close control.")
        try tap(close[0], app: app)
        let searchDismissed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            navigation.buttons.matching(identifier: "BackButton").allElementsBoundByIndex.filter(\.isHittable).count == 1
                && !navigation.buttons["Close"].exists && !app.keyboards.firstMatch.exists
                && app.buttons["settingsNetwork_bitcoinsv"].exists
        }, object: nil)
        try require(XCTWaiter.wait(for: [searchDismissed], timeout: 10) == .completed, "Closing native search must restore the original Networks navigation bar and BSV content without retaining the keyboard.")
        try capture("networks-search-dismissed", app: app, configuration: configuration, anchor: app.buttons["settingsNetwork_bitcoinsv"])
    }

    private func requireEmptySearch(_ field: XCUIElement) throws {
        switch field.value {
        case nil:
            break
        case let value as String:
            try require(value.isEmpty || value == field.placeholderValue, "A pre-existing or unexpected search value must never be erased by this public matrix.")
        default:
            throw Failure.publicRoute
        }
    }

    private func configuration() throws -> Configuration {
        let environment = ProcessInfo.processInfo.environment
        guard environment["BSVHOME_ALIGNMENT_STAGE8_UI"] == "1" else {
            throw XCTSkip("Requires explicit public-only phase-eight matrix opt-in on an already provisioned authorized QA resource.")
        }
        #if DEBUG && targetEnvironment(simulator)
            guard let role = environment["BSVHOME_ALIGNMENT_STAGE8_UI_ROLE"].flatMap(Role.init(rawValue:)),
                  environment["SIMULATOR_UDID"]?.uppercased() == role.simulatorId,
                  let language = environment["BSVHOME_ALIGNMENT_STAGE8_UI_LANGUAGE"].flatMap(Language.init(rawValue:)),
                  let variant = Variant(rawValue: environment["BSVHOME_ALIGNMENT_STAGE8_UI_VARIANT"] ?? "standard")
            else { throw Failure.invalidConfiguration }
            return Configuration(role: role, language: language, variant: variant, copy: Copy.forLanguage(language))
        #else
            throw Failure.invalidConfiguration
        #endif
    }

    private func publicPage(_ app: XCUIApplication) throws {
        try require(app.state == .runningForeground, "Public route verification requires the foreground App.")
        try require(!app.secureTextFields.firstMatch.exists && !app.alerts.firstMatch.exists, "Secret/authentication/system prompts are outside this read-only public matrix.")
        for identifier in ["word_0", "verifyWalletBackup", "continueWalletSecurityReminder", "continueWalletBackup", "transferPasswordCurrent", "transferPasswordNew", "transferPasswordConfirmation", "transferPasswordLoading", "transferPasswordSubmit", "transferPasswordCancel"] {
            try require(!app.descendants(matching: .any).matching(identifier: identifier).firstMatch.exists, "A secret or password workflow must never be entered or captured by this matrix.")
        }
    }

    private func present(_ element: XCUIElement) throws {
        try require(element.waitForExistence(timeout: 30), "The expected public route anchor is unavailable; no screenshot is requested.")
    }

    private func reveal(_ element: XCUIElement, app: XCUIApplication) throws {
        try present(element)
        for _ in 0 ..< 6 {
            if element.isHittable {
                return
            }
            try publicPage(app)
            app.swipeUp()
        }
        try require(element.isHittable, "The expected public control must remain reachable at this actual viewport and text size.")
    }

    private func tap(_ element: XCUIElement, app: XCUIApplication) throws {
        try publicPage(app)
        try reveal(element, app: app)
        try require(element.isEnabled, "Only the intended enabled public route may be opened.")
        element.tap()
        try publicPage(app)
    }

    private func nativeBack(_ app: XCUIApplication, title: String, returningTo element: XCUIElement) throws {
        try publicPage(app)
        let query = app.navigationBars.matching(NSPredicate(format: "identifier == %@ OR label == %@", title, title))
        let visible = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in query.allElementsBoundByIndex.contains(where: \.isHittable) }, object: nil)
        try require(XCTWaiter.wait(for: [visible], timeout: 15) == .completed, "The current public navigation bar must be present.")
        guard let bar = query.allElementsBoundByIndex.first(where: \.isHittable) else { throw Failure.publicRoute }
        let buttons = bar.buttons.matching(identifier: "BackButton").allElementsBoundByIndex.filter(\.isHittable)
        try require(buttons.count == 1, "Only the uniquely observed native BackButton may close this public page.")
        buttons[0].tap()
        try present(element)
        try publicPage(app)
    }

    private func capture(_ page: String, app: XCUIApplication, configuration: Configuration, anchor: XCUIElement) throws {
        try present(anchor)
        try publicPage(app)
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = "phase8-\(configuration.role.rawValue)-\(configuration.language.rawValue)-\(configuration.variant.rawValue)-\(page)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func require(_ condition: Bool, _ message: String) throws {
        guard condition else {
            XCTFail(message)
            throw Failure.publicRoute
        }
    }

    private enum Failure: Error {
        case invalidConfiguration, publicRoute
    }

    private enum Role: String {
        case unifiedSigner, observer, compactPublic

        var simulatorId: String {
            switch self {
            case .unifiedSigner: "F47186EF-B7A9-4956-A2A2-C04663361205"
            case .observer: "4A910C7A-C540-40C9-9405-04F46F5E81C1"
            case .compactPublic: "165BFA41-D46D-47DD-930E-698FDA23B0E1"
            }
        }

        var walletNames: [String] {
            switch self {
            case .unifiedSigner: ["Alignment Cold Signer QA"]
            case .observer: ["Alignment Cold Observer QA", "Alignment Legacy Observer Protocol"]
            case .compactPublic: ["Phase8 Compact Observer"]
            }
        }
    }

    private enum Language: String {
        case english = "en", chinese = "zh-Hans", german = "de", arabic = "ar"
    }

    private enum Variant: String {
        case standard, dark, large, compact
    }

    private struct Configuration {
        let role: Role
        let language: Language
        let variant: Variant
        let copy: Copy
    }

    private struct Copy {
        let locale: String
        let preferences: String
        let networks: String
        let status: String
        let contacts: String
        let security: String
        let onlinePassword: String
        let offlinePassword: String
        let emptyNetworks: String

        static func forLanguage(_ language: Language) -> Copy {
            switch language {
            case .english:
                Copy(locale: "en_US", preferences: "Preferences", networks: "Networks", status: "Status", contacts: "Contacts", security: "Security", onlinePassword: "Transfer Password", offlinePassword: "Offline Signing Password", emptyNetworks: "No networks found")
            case .chinese:
                Copy(locale: "zh_CN", preferences: "偏好设置", networks: "网络", status: "状态", contacts: "联系人", security: "安全设置", onlinePassword: "转账密码", offlinePassword: "离线签名密码", emptyNetworks: "未找到任何网络")
            case .german:
                Copy(locale: "de_DE", preferences: "Einstellungen", networks: "Netzwerke", status: "Status", contacts: "Kontakte", security: "Sicherheit", onlinePassword: "Überweisungspasswort", offlinePassword: "Passwort für Offline-Signaturen", emptyNetworks: "Keine Netzwerke gefunden")
            case .arabic:
                Copy(locale: "ar_SA", preferences: "التفضيلات", networks: "الشبكات", status: "حالة", contacts: "جهات الاتصال", security: "حماية", onlinePassword: "كلمة مرور التحويل", offlinePassword: "كلمة مرور التوقيع دون اتصال", emptyNetworks: "لم يتم العثور على شبكات")
            }
        }
    }
}
