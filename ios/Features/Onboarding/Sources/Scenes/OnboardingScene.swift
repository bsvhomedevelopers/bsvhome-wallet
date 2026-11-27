// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

public struct OnboardingScene: View {
    @Binding private var isPresentingCreateWalletSheet: Bool
    @Binding private var isPresentingImportWalletSheet: Bool

    public init(
        isPresentingCreateWalletSheet: Binding<Bool>,
        isPresentingImportWalletSheet: Binding<Bool>,
    ) {
        _isPresentingCreateWalletSheet = isPresentingCreateWalletSheet
        _isPresentingImportWalletSheet = isPresentingImportWalletSheet
    }

    public var body: some View {
        NavigationStack {
            ImportWalletWelcomeScene(
                onImportWallet: { isPresentingImportWalletSheet = true },
                onCreateWallet: { isPresentingCreateWalletSheet = true },
                presentation: .root,
            )
        }
    }
}
