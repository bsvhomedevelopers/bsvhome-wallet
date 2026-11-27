import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

struct AssetQueryScope: Hashable, Sendable {
    let chains: [String]
    let assetIds: [String]?

    init(environment: BsvHomeRuntimeEnvironment, allowedChains: [Chain]? = nil) {
        let available = environment.availableChains()
        chains = allowedChains.map { $0.map(\.rawValue).filter(available.contains) } ?? available
        assetIds = environment.allowedAssetIds()
    }

    func filter(assetId: Column, chain: Column) -> SQLExpression {
        let chainsFilter = chains.contains(chain).sqlExpression
        guard let assetIds else { return chainsFilter }
        return (chainsFilter && assetIds.contains(assetId)).sqlExpression
    }
}
