import Foundation
import enum BsvHomeCore.BsvHomeWalletImportKind
import Primitives

public enum ImportWalletInputKind: Hashable {
    case phrase
    case privateKey
    case address

    var importKind: BsvHomeWalletImportKind {
        switch self {
        case .phrase: .phrase
        case .privateKey: .privateKey
        case .address: .address
        }
    }

    func canImport(input: String, selectedWordCount: Int, selectedChain: Chain?) -> Bool {
        guard !input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return false }
        return switch self {
        case .phrase: input.split(whereSeparator: \.isWhitespace).count == selectedWordCount
        case .privateKey, .address: selectedChain != nil
        }
    }
}
