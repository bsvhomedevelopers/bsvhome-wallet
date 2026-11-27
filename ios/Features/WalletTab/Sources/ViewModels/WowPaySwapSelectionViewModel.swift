import Observation

@Observable
@MainActor
final class WowPaySwapSelectionViewModel {
    enum Side {
        case from, to
    }

    private(set) var from: WowPayAsset = .bsv
    private(set) var to: WowPayAsset = .usdt
    var side: Side?

    var selectedAsset: WowPayAsset? {
        switch side {
        case .from: from
        case .to: to
        case nil: nil
        }
    }

    func selectPair(from: WowPayAsset, to: WowPayAsset) {
        self.from = from
        self.to = to
    }
}
