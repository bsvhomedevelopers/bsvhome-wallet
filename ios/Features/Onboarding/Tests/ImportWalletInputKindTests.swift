@testable import Onboarding
import Primitives
import Testing

struct ImportWalletInputKindTests {
    @Test(arguments: [15, 18, 21])
    func phraseRequiresSelectedWordCount(_ count: Int) {
        let words = Array(repeating: "sample", count: count).joined(separator: " \n ")

        #expect(ImportWalletInputKind.phrase.canImport(input: words, selectedWordCount: count, selectedChain: nil))
        #expect(!ImportWalletInputKind.phrase.canImport(input: words, selectedWordCount: 12, selectedChain: nil))
    }

    @Test
    func privateKeyRequiresNetwork() {
        #expect(!ImportWalletInputKind.privateKey.canImport(input: "example", selectedWordCount: 12, selectedChain: nil))
        #expect(ImportWalletInputKind.privateKey.canImport(input: "example", selectedWordCount: 12, selectedChain: .ethereum))
    }

    @Test
    func observationRequiresNetwork() {
        #expect(!ImportWalletInputKind.address.canImport(input: "example", selectedWordCount: 12, selectedChain: nil))
        #expect(ImportWalletInputKind.address.canImport(input: "example", selectedWordCount: 12, selectedChain: .bitcoin))
    }
}
