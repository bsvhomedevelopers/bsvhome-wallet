import BsvHomeCorePrimitives
import Primitives

enum AssetDetailLayout {
    case asset
    case token

    init(assetId: AssetId) {
        let usdtAssetIds = [Chain.tron, .tronShasta].map { $0.defaultAsset(type: .trc20).id }
        self = usdtAssetIds.contains(assetId) ? .token : .asset
    }
}
