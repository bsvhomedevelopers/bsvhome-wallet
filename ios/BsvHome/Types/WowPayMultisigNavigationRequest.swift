import Foundation
import enum BsvHomeCore.BsvHomeBsvMultisigPublicFundingSource

struct WowPayMultisigNavigationRequest: Identifiable, Sendable {
    enum Destination: Sendable {
        case publicFunding(BsvHomeBsvMultisigPublicFundingSource)
        case invitation(String)
        case collectiveSend(signerWalletId: String, walletId: String, code: String)
    }

    enum NavigationMode: Equatable, Sendable {
        case push, replaceStack
    }

    let id = UUID()
    let destination: Destination
    let navigationMode: NavigationMode
    let isActive: @MainActor () -> Bool

    init(destination: Destination, navigationMode: NavigationMode = .replaceStack, isActive: @escaping @MainActor () -> Bool = { true }) {
        self.destination = destination
        self.navigationMode = navigationMode
        self.isActive = isActive
    }

    func cancel() {
        if case let .publicFunding(source) = destination {
            source.cancelPaymailPreparation()
        }
    }
}
