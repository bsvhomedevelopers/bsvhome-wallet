@testable import BsvColdWallet
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletStorageProtectionTests {
    @Test
    func protectionFailurePreservesImportedWalletAndBlocksSensitiveActions() async {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline, protectStorage: { throw BsvHomeServiceError.Store(msg: "fixture protection failure") })
        model.walletName = "Fixture"
        model.words = ["public", "fixture"]
        model.isBackupConfirmed = true
        model.create()
        await model.operationTask?.value
        #expect(model.wallet?.id == BsvColdWalletServiceMock.fixture.wallet.id)
        #expect(model.wallets.contains { $0.id == BsvColdWalletServiceMock.fixture.wallet.id })
        #expect(model.storageProtectionFailed)
        #expect(model.path.isEmpty)
        #expect(model.words.isEmpty)
        #expect(model.errorMessage != nil)
        #expect(!model.canSign)
        #expect(!model.canImport)
    }
}
