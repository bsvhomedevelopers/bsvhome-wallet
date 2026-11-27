import Foundation
import protocol BsvHomeCore.BsvHomeWalletServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Primitives
import SwiftUI

public struct ImportWalletTypeViewModel {
    private let service: any BsvHomeWalletServiceProtocol

    public init(service: any BsvHomeWalletServiceProtocol) {
        self.service = service
    }

    func filterChains(for query: String) -> [Chain] {
        service.importChains(kind: .phrase, query: query).map { Chain(core: $0) }
    }

    var title: String {
        Localized.Wallet.Import.title
    }

    func items(for searchText: String) -> [Chain] {
        filterChains(for: searchText)
    }
}

// MARK: - Equatable

extension ImportWalletTypeViewModel: Equatable {
    public static func == (lhs: ImportWalletTypeViewModel, rhs: ImportWalletTypeViewModel) -> Bool {
        lhs.filterChains(for: "") == rhs.filterChains(for: "")
    }
}

// MARK: - Hashable

extension ImportWalletTypeViewModel: Hashable {
    public func hash(into hasher: inout Hasher) {
        hasher.combine(filterChains(for: ""))
    }
}
