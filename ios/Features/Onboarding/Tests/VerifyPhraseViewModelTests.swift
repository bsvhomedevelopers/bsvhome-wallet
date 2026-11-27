// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeServiceError
@testable import Onboarding
import Primitives
import Testing

@MainActor
struct VerifyPhraseViewModelTests {
    private let words = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu"]

    @Test
    func failedCreationReEnablesTheButtonAndShowsTheError() async throws {
        let model = VerifyPhraseViewModel(words: words) { _ in throw AnyError("keystore write failed") }
        try verifyPhrase(model)

        await model.complete()

        #expect(model.buttonState == .normal)
        #expect(model.isPresentingAlertMessage?.message == AnyError("keystore write failed").localizedDescription)
    }

    @Test
    func cancelledPromptReEnablesTheButtonWithoutAnError() async throws {
        let model = VerifyPhraseViewModel(words: words) { _ in throw BsvHomeServiceError.Cancelled }
        try verifyPhrase(model)

        await model.complete()

        #expect(model.buttonState == .normal)
        #expect(model.isPresentingAlertMessage == nil)
    }

    @Test
    func successfulCreationKeepsTheButtonBusyWhileTheFlowMovesOn() async throws {
        var matchedPhrase = false
        let model = VerifyPhraseViewModel(words: words) { matchedPhrase = $0 == words }
        try verifyPhrase(model)

        await model.complete()

        #expect(model.buttonState == .loading(showProgress: true))
        #expect(model.isPresentingAlertMessage == nil)
        #expect(matchedPhrase)
    }

    @Test(arguments: [0, 1, 11])
    func incompletePhraseCannotComplete(verifiedCount: Int) async throws {
        var completions = 0
        let model = VerifyPhraseViewModel(words: words) { _ in completions += 1 }
        for word in words.prefix(verifiedCount) {
            try model.pickWord(index: #require(model.selectRows.first { $0.word == word }))
        }

        model.onContinue()
        await model.complete()

        #expect(completions == 0)
        #expect(model.buttonState == .disabled)
    }

    @Test
    func incorrectOrderCannotComplete() async throws {
        var completions = 0
        let model = VerifyPhraseViewModel(words: words) { _ in completions += 1 }
        for word in words.reversed() {
            try model.pickWord(index: #require(model.selectRows.first { $0.word == word }))
        }

        model.onContinue()
        await model.complete()

        #expect(completions == 0)
        #expect(model.buttonState == .disabled)
    }

    @Test
    func changedVerifiedOrderCannotComplete() async throws {
        var completions = 0
        let model = VerifyPhraseViewModel(words: words) { _ in completions += 1 }
        try verifyPhrase(model)
        model.wordsVerified.swapAt(0, 1)

        model.onContinue()
        await model.complete()

        #expect(completions == 0)
    }

    @Test(arguments: [4, 15, 24])
    func aDifferentPhraseLengthCannotComplete(count: Int) async throws {
        var completions = 0
        let fixtureWords = (0 ..< count).map { "fixture-\($0)" }
        let model = VerifyPhraseViewModel(words: fixtureWords) { _ in completions += 1 }
        for word in fixtureWords {
            try model.pickWord(index: #require(model.selectRows.first { $0.word == word }))
        }

        model.onContinue()
        await model.complete()

        #expect(completions == 0)
        #expect(model.buttonState == .disabled)
    }

    private func verifyPhrase(_ model: VerifyPhraseViewModel) throws {
        for word in words {
            try model.pickWord(index: #require(model.selectRows.first { $0.word == word }))
        }
    }
}
