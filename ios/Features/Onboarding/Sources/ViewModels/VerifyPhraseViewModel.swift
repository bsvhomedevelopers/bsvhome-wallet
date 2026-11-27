// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import BsvHomeCorePrimitives
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

@Observable
@MainActor
final class VerifyPhraseViewModel {
    private let words: [String]
    private let shuffledWords: [String]
    private let onComplete: ([String]) async throws -> Void

    var wordsVerified: [String]
    var wordsIndex: Int = 0
    var buttonState = ButtonState.disabled
    var isPresentingAlertMessage: AlertMessage?
    private var selectedIndexes = Set<WordIndex>()

    init(
        words: [String],
        onComplete: @escaping ([String]) async throws -> Void,
    ) {
        self.words = words
        shuffledWords = words.shuffleInGroups(groupSize: 4)
        wordsVerified = Array(repeating: "", count: words.count)
        self.onComplete = onComplete
    }

    var title: String {
        Localized.VerifyPhrase.title
    }

    var docsUrl: URL {
        AppUrl.docs(.howToSecureSecretPhrase)
    }

    private var hasVerifiedPhrase: Bool {
        words.count == 12 && wordsIndex == words.count && wordsVerified == words
    }

    var rows: [[WordIndex]] {
        wordsVerified
            .enumerated()
            .map {
                WordIndex(index: $0.offset, word: $0.element)
            }
            .splitInSubArrays(into: wordsVerified.count / 2)
    }

    var rowsSections: [[WordIndex]] {
        selectRows.chunks(4)
    }

    var selectRows: [WordIndex] {
        shuffledWords
            .enumerated()
            .map {
                WordIndex(index: $0.offset, word: $0.element)
            }
    }

    func pickWord(index: WordIndex) {
        guard wordsIndex < words.count else { return }
        if words[wordsIndex] == index.word {
            wordsVerified[wordsIndex] = index.word
            selectedIndexes.insert(index)
            wordsIndex += 1
        }
        if hasVerifiedPhrase {
            buttonState = .normal
        }
    }

    func isVerified(index: WordIndex) -> Bool {
        selectedIndexes.contains(index)
    }
}

// MARK: - Actions

extension VerifyPhraseViewModel {
    func onContinue() {
        guard hasVerifiedPhrase else { return }
        if case .loading = buttonState {
            return
        }
        buttonState = .loading(showProgress: true)
        Task {
            await complete()
        }
    }

    func complete() async {
        guard hasVerifiedPhrase else { return }
        buttonState = .loading(showProgress: true)
        do {
            try await onComplete(words)
        } catch {
            buttonState = .normal
            isPresentingAlertMessage = AlertMessage(title: Localized.Errors.createWallet(""), error: error)
        }
    }
}
