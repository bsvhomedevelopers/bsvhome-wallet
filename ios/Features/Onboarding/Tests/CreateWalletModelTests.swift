// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeWalletService
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
@testable import Onboarding
import Primitives
import Store
import StoreTestKit
import Testing

@MainActor
struct CreateWalletModelTests {
    @Test
    func createWalletSetsWalletConfiguration() async throws {
        let model = CreateWalletModel(
            service: BsvHomeWalletService.mock(db: .mockWithChains(AssetConfiguration.allChains)),
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: nil,
        )

        let created = try await model.createWallet(words: LocalKeystore.words)
        #expect(created.wallet.source == .create)
        #expect(created.hasExistingWallets == false)
    }

    @Test
    func generateSecretPhraseReturnsGeneratedWords() throws {
        let model = CreateWalletModel(
            service: BsvHomeWalletService.mock(),
            preferences: .mock(),
            avatarService: BsvHomeAvatarServiceMock(),
            onComplete: nil,
        )

        let words = try model.generateSecretPhrase()
        let nextWords = try model.generateSecretPhrase()

        #expect(words.count == 12)
        #expect(words != nextWords)
    }
}
