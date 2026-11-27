import Style
import SwiftUI

enum BsvMultisigNoticeTone {
    case information
    case warning
    case error

    var color: Color {
        switch self {
        case .information: Colors.WowPay.multisigNoticeText
        case .warning: Colors.WowPay.multisigRiskText
        case .error: Colors.WowPay.multisigErrorText
        }
    }

    var background: Color {
        switch self {
        case .information: Colors.WowPay.multisigNoticeSurface
        case .warning: Colors.WowPay.multisigRiskSurface
        case .error: Colors.WowPay.multisigErrorSurface
        }
    }
}
