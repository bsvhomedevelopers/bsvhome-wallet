@testable import BsvColdWallet
import Components
import SwiftUI
import Testing

@MainActor
struct BsvColdWalletNavigationTests {
    @Test
    func internalPushesPreserveTheImportContextAndRemovingTheFeatureClearsIt() async throws {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline)
        let refresh = Refresh()
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = UIHostingController(rootView: Host(model: model, refresh: refresh))
        window.makeKeyAndVisible()
        defer { window.isHidden = true; window.rootViewController = nil }
        try await settle()
        await model.operationTask?.value
        try #require(model.isForeground)
        try #require(!model.isBusy)
        model.open(.recover)
        try await settle()
        #expect(model.path == [.recover])
        model.walletName = "Public fixture"
        model.input = "public fixture input"
        model.passphrase = "public fixture passphrase"
        model.previewImport()
        await model.operationTask?.value
        try await settle()
        #expect(model.path == [.recover, .importReview])
        #expect(model.previewAddress == "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA")
        #expect(!model.input.isEmpty)
        #expect(!model.passphrase.isEmpty)
        refresh.revision += 1
        try await settle()
        #expect(model.path == [.recover, .importReview])
        #expect(!model.input.isEmpty)
        #expect(!model.passphrase.isEmpty)
        refresh.phase = .inactive
        try await settle()
        #expect(!model.isForeground)
        #expect(model.path.last == .importReview)
        refresh.phase = .background
        try await settle()
        #expect(model.path.isEmpty)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        refresh.phase = .active
        try await settle()
        await model.operationTask?.value
        try #require(!model.isBusy)
        model.open(.recover)
        model.walletName = "Public fixture"
        model.input = "public fixture input"
        model.previewImport()
        await model.operationTask?.value
        try await settle()
        #expect(model.path.last == .importReview)
        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        try await settle()
        #expect(model.path.isEmpty)
        #expect(model.input.isEmpty)
        model.input = "public fixture input before exit"
        window.rootViewController = nil
        try await settle()
        #expect(model.path.isEmpty)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(500))
    }

    @Observable
    final class Refresh {
        var revision = 0
        var phase: ScenePhase = .active
    }

    struct Host: View {
        let model: BsvColdWalletViewModel
        let refresh: Refresh
        @State private var navigation = NavigationPathState()
        @State private var isInstalled = false

        var body: some View {
            NavigationStack(path: navigation.binding) {
                BsvColdWalletScene(model: model, navigation: navigation)
                    .environment(\.locale, Locale(identifier: refresh.revision.isMultiple(of: 2) ? "en_US" : "zh_Hans"))
                    .environment(\.scenePhase, refresh.phase)
                    .onAppear { isInstalled = true }
                    .bsvColdWalletNavigation(model: isInstalled ? model : nil, navigation: navigation)
            }
        }
    }
}
