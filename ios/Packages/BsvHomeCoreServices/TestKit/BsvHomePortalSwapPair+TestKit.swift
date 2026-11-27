import struct BsvHomeCore.BsvHomePortalSwapAsset
import enum BsvHomeCore.BsvHomePortalSwapDirection
import struct BsvHomeCore.BsvHomePortalSwapPair
import BsvHomeCorePrimitives
import Primitives

public extension BsvHomePortalSwapPair {
    static func mock(direction: BsvHomePortalSwapDirection = .bsvToUsdt) -> BsvHomePortalSwapPair {
        let bsv = BsvHomePortalSwapAsset(assetId: AssetId(chain: .bitcoinSv).identifier, symbol: "BSV", network: "Bitcoin SV", scale: 8)
        let usdt = BsvHomePortalSwapAsset(assetId: Chain.tron.defaultAsset(type: .trc20).id.identifier, symbol: "USDT", network: "TRON / TRC20", scale: 6)
        let trx = BsvHomePortalSwapAsset(assetId: Chain.tron.asset.id.identifier, symbol: "TRX", network: "TRON", scale: 6)
        let assets: (BsvHomePortalSwapAsset, BsvHomePortalSwapAsset) = switch direction {
        case .bsvToUsdt: (bsv, usdt)
        case .usdtToBsv: (usdt, bsv)
        case .bsvToTrx: (bsv, trx)
        case .trxToBsv: (trx, bsv)
        case .usdtToTrx: (usdt, trx)
        case .trxToUsdt: (trx, usdt)
        }
        return BsvHomePortalSwapPair(direction: direction, from: assets.0, to: assets.1, providerDisplayName: "Provider")
    }
}
