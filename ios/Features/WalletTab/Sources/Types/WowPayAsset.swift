import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style
import SwiftUI

public enum WowPayAsset: String, CaseIterable, Identifiable, Sendable {
    case bsv = "BSV"
    case trx = "TRX"
    case usdt = "USDT"
    case mnee = "MNEE"

    public static let visibleCases: [Self] = {
        let allowed = BsvHomeRuntimeEnvironment.current.allowedAssetIds()
        return allCases.filter { asset in
            guard let id = asset.assetId?.identifier else { return false }
            return allowed?.contains(id) ?? true
        }
    }()

    static let bsvAssetId = AssetId(chain: .bitcoinSv)
    static let trxAssetId = Chain.tron.asset.id
    static let usdtAssetId = Chain.tron.defaultAsset(type: .trc20).id

    public var id: Self { self }
    var assetId: AssetId? {
        switch self {
        case .bsv: Self.bsvAssetId
        case .trx: Self.trxAssetId
        case .usdt: Self.usdtAssetId
        case .mnee: nil
        }
    }

    var image: Image {
        switch self {
        case .trx: Images.Chains.tron
        case .bsv, .usdt, .mnee: Image("wowpay-\(rawValue.lowercased())")
        }
    }

    var subtitle: String {
        switch self {
        case .bsv: "Bitcoin SV"
        case .trx: "TRON"
        case .usdt: "Tether USD"
        case .mnee: Localized.wowpayMneeDescription
        }
    }
}
