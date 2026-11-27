// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

extension XCTestCase {
    @MainActor
    func recordScreen(_ name: String, app: XCUIApplication) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    @MainActor
    func setupPermissionHandler() {
        addUIInterruptionMonitor(withDescription: "Permission Alert") { alert in
            let allowButton = alert.buttons["Allow"]
            if allowButton.exists {
                allowButton.tap()
                return true
            }
            return false
        }
    }
}
