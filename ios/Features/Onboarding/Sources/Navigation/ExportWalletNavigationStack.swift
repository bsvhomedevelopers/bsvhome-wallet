// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeWalletSecret
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

enum ExportWalletDestination: Hashable {
    case words
    case privateKey
    case recovery
}

public struct ExportWalletNavigationStack: View {
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.dismiss) private var dismiss
    @State private var flow: BsvHomeWalletSecret?
    @State private var navigationPath: NavigationPath = .init()

    public init(flow: BsvHomeWalletSecret) {
        _flow = State(initialValue: flow)
    }

    public var body: some View {
        NavigationStack(path: $navigationPath) {
            let title = switch flow {
            case .words: Localized.Common.secretPhrase
            case .privateKey: Localized.Common.privateKey
            case .recovery: Localized.bsvHdBackupTitle
            case nil: Localized.bsvHdBackupUnavailable
            }
            SecurityReminderScene(
                model: SecurityReminderViewModel(
                    title: title,
                    onNext: onNext,
                ),
            )
            .toolbarDismissItem(type: .close, placement: .topBarLeading)
            .navigationBarTitleDisplayMode(.inline)
            .navigationDestination(for: ExportWalletDestination.self) {
                switch $0 {
                case .words:
                    if case let .words(words) = flow {
                        ShowSecretDataScene(model: ShowSecretPhraseViewModel(words: words))
                    }
                case .privateKey:
                    if case let .privateKey(key) = flow {
                        ShowSecretDataScene(model: ShowPrivateKeyViewModel(text: key))
                    }
                case .recovery:
                    if case let .recovery(recovery) = flow {
                        RecoveryBackupScene(recovery: recovery)
                    }
                }
            }
        }
        .overlay {
            if scenePhase != .active {
                Colors.WowPay.background.ignoresSafeArea()
            }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                clear()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.userDidTakeScreenshotNotification)) { _ in clear() }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.protectedDataWillBecomeUnavailableNotification)) { _ in clear() }
        .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in
            if UIScreen.main.isCaptured {
                clear()
            }
        }
        .onDisappear { flow = nil }
        .protectFromScreenRecording()
    }
}

extension ExportWalletNavigationStack {
    private func onNext() {
        switch flow {
        case .words:
            navigationPath.append(ExportWalletDestination.words)
        case .privateKey:
            navigationPath.append(ExportWalletDestination.privateKey)
        case .recovery:
            navigationPath.append(ExportWalletDestination.recovery)
        case nil:
            break
        }
    }

    private func clear() {
        flow = nil
        navigationPath = NavigationPath()
        dismiss()
    }
}
