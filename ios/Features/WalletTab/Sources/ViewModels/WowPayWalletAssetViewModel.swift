import Components
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

struct WowPayWalletAssetViewModel: ListAssetItemViewable {
    let asset: WowPayAsset
    let assetData: AssetData?
    let showBalancePrivacy: Binding<Bool>
    var action: ((ListAssetItemAction) -> Void)?

    private let dataModel: AssetDataViewModel?

    init(asset: WowPayAsset, assetData: AssetData?, currencyCode: String, showBalancePrivacy: Binding<Bool>) {
        self.asset = asset
        self.assetData = assetData
        self.showBalancePrivacy = showBalancePrivacy
        dataModel = assetData.map {
            AssetDataViewModel(assetData: $0, formatter: .full, currencyCode: currencyCode)
        }
    }

    var name: String { asset.rawValue }
    var symbol: String? { nil }
    var assetImage: AssetImage { .image(asset.image) }
    var subtitleView: ListAssetItemSubtitleView {
        .type(TextValue(text: asset.subtitle, style: .calloutSecondary))
    }

    var balanceText: String { dataModel?.balanceText ?? "—" }
    var fiatBalanceText: String {
        guard let dataModel, dataModel.isPriceAvailable else { return "—" }
        let text = dataModel.fiatBalanceText
        return text.isEmpty ? dataModel.priceViewModel.fiatAmountText(amount: 0) : text
    }

    var rightView: ListAssetItemRightView {
        .balance(
            balance: TextValue(text: balanceText, style: TextStyle(font: .wowPay.assetValue, color: Colors.WowPay.heading)),
            totalFiat: TextValue(text: fiatBalanceText, style: .calloutSecondary),
        )
    }
}
