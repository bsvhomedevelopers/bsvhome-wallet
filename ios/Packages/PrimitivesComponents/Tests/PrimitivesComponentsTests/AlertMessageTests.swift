// Copyright (c). Gem Wallet. All rights reserved.

import Components
import enum BsvHomeCore.BsvHomeServiceError
import enum BsvHomeCore.BsvHomeWalletImportError
import Localization
import Primitives
@testable import PrimitivesComponents
import Testing

struct AlertMessageTests {
    @Test(arguments: [
        BsvHomeWalletImportError.InvalidSecretPhrase,
        .InvalidSecretPhraseWords(words: ["not-a-mnemonic-word"]),
        .InvalidPrivateKey,
        .InvalidAddress,
        .MissingChain,
    ])
    func importValidationKeepsItsLocalizedDescription(error: BsvHomeWalletImportError) {
        let alert = AlertMessage(error: BsvHomeServiceError.WalletImport(error: error))

        #expect(alert?.message == error.localizedDescription)
    }

    @Test
    func importValidationUsesTheExistingTranslation() {
        let alert = AlertMessage(error: BsvHomeServiceError.WalletImport(error: .InvalidSecretPhrase))

        #expect(alert?.message == Localized.Errors.Import.invalidSecretPhrase)
    }

    @Test
    func bsvApiUnsupportedIsLocalizedWithoutChangingNetworkErrors() {
        #expect(BsvHomeServiceError.Unsupported(msg: "BSV_API_UNSUPPORTED: chart").localizedDescription == Localized.Errors.Connections.unsupportedChain)
        #expect(BsvHomeServiceError.Gateway(msg: "HTTP error: status 429").localizedDescription == "HTTP error: status 429")
    }

    @Test
    func failureBecomesAnAlertWithItsDescription() {
        let alert = AlertMessage(title: "Create", error: AnyError("disk full"))

        #expect(alert?.title == "Create")
        #expect(alert?.message == AnyError("disk full").localizedDescription)
    }

    @Test
    func cancellationIsNotAnAlert() {
        #expect(AlertMessage(error: BsvHomeServiceError.Cancelled) == nil)
        #expect(AlertMessage(error: CancellationError()) == nil)
    }
}
