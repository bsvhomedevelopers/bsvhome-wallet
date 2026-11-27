import Style
import SwiftUI

public enum CurrencyInputStyle {
    case standard
    case wowPaySwap
    case wowPayTransfer

    var font: WowPayFont {
        switch self {
        case .standard: .native(.app.display)
        case .wowPaySwap, .wowPayTransfer: .wowPay.amount
        }
    }

    var foregroundColor: Color {
        switch self {
        case .standard: Colors.black
        case .wowPaySwap, .wowPayTransfer: Colors.WowPay.heading
        }
    }

    var textAlignment: TextAlignment {
        switch self {
        case .standard: .center
        case .wowPaySwap: .trailing
        case .wowPayTransfer: .leading
        }
    }

    var maximumWidth: CGFloat {
        switch self {
        case .standard: 210
        case .wowPaySwap: Sizing.wowPay.swapAmountMaxWidth
        case .wowPayTransfer: .infinity
        }
    }

    func height(scaledFontSize: CGFloat) -> CGFloat {
        switch self {
        case .standard: UIFont.systemFont(ofSize: 44, weight: .semibold).lineHeight
        case .wowPaySwap, .wowPayTransfer: UIFont.systemFont(ofSize: scaledFontSize, weight: .bold).lineHeight
        }
    }
}
