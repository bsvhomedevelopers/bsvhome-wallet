import Foundation
@testable import OfflineIdentity
import SwiftUI
import Testing
import UIKit

@MainActor
struct OfflineIdentityPrivacyTests {
    @Test
    func internalNavigationPreservesInputAndCaptureAndFeatureExitClearIt() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let model = OfflineIdentityViewModel(keychain: OfflineIdentityKeychainMock(), rootURL: root, authenticate: {})
        model.sceneChanged(.active)
        let host: UIHostingController<AnyView> = UIHostingController(rootView: AnyView(OfflineIdentityScene(model: model).environment(\.scenePhase, .active)))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.rootViewController = nil; window.isHidden = true }
        try await settle()
        model.password = "synthetic-test-password"
        model.path = [.backup]
        try await settle()
        #expect(model.password == "synthetic-test-password")
        model.captureChanged(true)
        #expect(model.password.isEmpty)
        model.captureChanged(false)
        model.password = "synthetic-test-password"
        host.rootView = AnyView(Text(""))
        try await settle()
        #expect(model.password.isEmpty)
        #expect(model.challengeJSON.isEmpty)
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(500))
    }
}
