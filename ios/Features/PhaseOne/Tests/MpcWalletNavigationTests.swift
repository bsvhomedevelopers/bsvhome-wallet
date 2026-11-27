import Observation
@testable import PhaseOne
import SwiftUI
import Testing

@MainActor
@Suite(.serialized)
struct MpcWalletNavigationTests {
    @Test
    func rebuildingParentPreservesBackupButLeavingItsPageClearsRecoveryKey() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        let state = NavigationState()
        let (window, previousRoot) = try await hostWindow()
        window.rootViewController = UIHostingController(rootView: Host(model: model, state: state))
        window.makeKeyAndVisible()
        defer { window.rootViewController = previousRoot; window.makeKeyAndVisible() }
        for _ in 0 ..< 50 {
            if !model.isBusy, !model.wallets.isEmpty {
                break
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        try #require(!model.isBusy)
        state.path = [.wallet("wallet-a"), .backup("wallet-a")]
        try await settle()
        try #require(model.selectedWalletId == "wallet-a")
        model.backup = MpcWalletServiceMock.material()
        model.recoveryKeyConfirmation = "synthetic fixture confirmation"
        state.variant.toggle()
        try await settle()
        #expect(state.path == [.wallet("wallet-a"), .backup("wallet-a")])
        #expect(model.backup != nil)
        #expect(!model.recoveryKeyConfirmation.isEmpty)
        state.path.removeLast()
        for _ in 0 ..< 50 {
            if model.backup == nil {
                break
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        #expect(model.backup == nil)
        #expect(model.recoveryKeyConfirmation.isEmpty)
    }

    @Test
    func removingTheBackupPresenterClosesItsRealPickerAndClearsRecoveryMaterial() async throws {
        let animationsEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(animationsEnabled) }
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        model.backup = MpcWalletServiceMock.material()
        model.recoveryKeyConfirmation = "synthetic fixture confirmation"
        let ticket = try #require(await model.makeBackupExport())
        defer { try? ticket.removeTemporaryFile() }
        let state = ExportState(ticket: ticket)
        let (window, previousRoot) = try await hostWindow()
        window.rootViewController = UIHostingController(rootView: ExportHost(model: model, state: state))
        window.makeKeyAndVisible()
        defer { window.rootViewController = previousRoot; window.makeKeyAndVisible() }
        for _ in 0 ..< 50 {
            if let presented = window.rootViewController?.presentedViewController,
               presented.viewIfLoaded?.window != nil
            {
                break
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        try #require(window.rootViewController?.presentedViewController != nil)
        try #require(window.rootViewController?.presentedViewController?.viewIfLoaded?.window != nil)
        #expect(model.backup != nil)
        #expect(!model.recoveryKeyConfirmation.isEmpty)
        #expect(FileManager.default.fileExists(atPath: ticket.fileURL.path))
        state.hasPresenter = false
        for _ in 0 ..< 50 {
            if window.rootViewController?.presentedViewController == nil,
               model.backup == nil,
               !FileManager.default.fileExists(atPath: ticket.fileURL.path)
            {
                break
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        #expect(window.rootViewController?.presentedViewController == nil)
        #expect(model.backup == nil)
        #expect(model.recoveryKeyConfirmation.isEmpty)
        #expect(!FileManager.default.fileExists(atPath: ticket.fileURL.path))
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(500))
    }

    private func hostWindow() async throws -> (UIWindow, UIViewController?) {
        for _ in 0 ..< 50 {
            if UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene && $0.activationState == .foregroundActive }) {
                break
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        let scene = try #require(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first { $0.activationState == .foregroundActive })
        let window = try #require(scene.windows.first(where: \.isKeyWindow) ?? scene.windows.first(where: { !$0.isHidden }))
        return (window, window.rootViewController)
    }

    @Observable
    final class NavigationState {
        var path: [MpcWalletRoute] = []
        var variant = false
    }

    @Observable
    final class ExportState {
        var ticket: MpcBackupExportTicket?
        var hasPresenter = true

        init(ticket: MpcBackupExportTicket) {
            self.ticket = ticket
        }
    }

    struct ExportHost: View {
        let model: MpcWalletViewModel
        @Bindable var state: ExportState

        var body: some View {
            Group {
                if state.hasPresenter {
                    Color.clear
                        .modifier(MpcBackupPageLifetime(model: model, walletId: "wallet-a", ticket: $state.ticket))
                } else {
                    Text("closed")
                }
            }
            .modifier(MpcBackupPresentation(model: model, ticket: $state.ticket))
        }
    }

    struct Host: View {
        let model: MpcWalletViewModel
        @Bindable var state: NavigationState

        var body: some View {
            NavigationStack(path: $state.path) {
                MpcWalletScene(model: model)
                    .environment(\.scenePhase, .active)
                    .environment(\.locale, Locale(identifier: state.variant ? "zh_Hans" : "en_US"))
            }
        }
    }
}
