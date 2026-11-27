@testable import BsvColdWallet
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletPrivacyTests {
    @Test
    func backgroundClearsTemporarySecretsAndAcknowledgments() {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline)
        model.words = ["public", "fixture"]
        model.input = "fixture recovery"
        model.passphrase = "fixture passphrase"
        model.backup = "fixture backup"
        model.isReviewConfirmed = true
        model.isPreviewConfirmed = true
        model.path = [.backup]
        model.sceneChanged(.background)
        #expect(model.words.isEmpty)
        #expect(model.input.isEmpty)
        #expect(model.passphrase.isEmpty)
        #expect(model.backup == nil)
        #expect(!model.isReviewConfirmed)
        #expect(!model.isPreviewConfirmed)
        #expect(model.path.isEmpty)
        #expect(!model.isForeground)
    }

    @Test
    func captureClearsPendingConfirmationAndBlocksSensitiveActions() {
        let model = BsvColdWalletViewModel(service: BsvColdWalletServiceMock(noHandle: .init()), mode: .offline)
        model.backup = "fixture backup"
        model.input = "fixture recovery"
        model.isReviewConfirmed = true
        model.snapshot = BsvColdWalletServiceMock.fixture
        model.startScan(.request)
        model.captureChanged(true)
        #expect(model.backup == nil)
        #expect(model.input.isEmpty)
        #expect(model.scanner == nil)
        #expect(!model.canSign)
        #expect(!model.canBroadcast)
    }
}
