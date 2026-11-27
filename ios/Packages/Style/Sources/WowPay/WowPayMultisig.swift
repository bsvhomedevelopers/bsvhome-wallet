import SwiftUI

public extension Colors.WowPay {
    static let multisigPositiveSurface = Color.dynamicColor("#E2F8EF", dark: "#173C34")
    static let multisigPositiveText = Color.dynamicColor("#07895A", dark: "#4BD4A2")
    static let multisigWarningSurface = Color.dynamicColor("#FFF3DB", dark: "#3B3020")
    static let multisigWarningText = Color.dynamicColor("#A67417", dark: "#E5C47C")
    static let multisigNoticeSurface = Color.dynamicColor("#EAF4FF", dark: "#233851")
    static let multisigNoticeText = Color.dynamicColor("#5C7094", dark: "#A4B6D5")
    static let multisigRiskSurface = Color.dynamicColor("#FFF5E6", dark: "#3B3020")
    static let multisigRiskText = Color.dynamicColor("#936718", dark: "#E5C47C")
    static let multisigBadge = Color.dynamicColor("#EC3947", dark: "#EC3947")
    static let multisigInputSurface = Color.dynamicColor("#F9FCFF", dark: "#1A263A")
    static let multisigErrorSurface = Color.dynamicColor("#FFF1EF", dark: "#462A29")
    static let multisigErrorText = Color.dynamicColor("#B74235", dark: "#FF9188")
    static let multisigCoin = Color.dynamicColor("#F0B51A", dark: "#F0B51A")
}

public extension WowPayFont.wowPay {
    static let multisigLabel: WowPayFont = .native(.footnote.weight(.bold))
    static let multisigValue: WowPayFont = .native(.footnote.weight(.semibold))
    static let multisigTab: WowPayFont = .native(.footnote.weight(.bold))
    static let multisigChip: WowPayFont = .native(.caption2.weight(.bold))
    static let multisigBannerTitle: WowPayFont = .native(.title2.weight(.bold))
    static let multisigTotal: WowPayFont = .native(.largeTitle.weight(.heavy))
    static let multisigTransactionAmount: WowPayFont = .scaled(size: 30, weight: .bold, relativeTo: .largeTitle)
}

public extension Spacing.wowPay {
    static let multisigMember: CGFloat = 14
    static let multisigField: CGFloat = 15
}

public extension Sizing.wowPay {
    static let multisigTabControlHeight: CGFloat = 40
    static let multisigAvatar: CGFloat = 36
    static let multisigPerson: CGFloat = 31
    static let multisigProgress: CGFloat = 6
    static let multisigBadge: CGFloat = 20
    static let multisigCoin: CGFloat = 39
    static let multisigPaymentQR: CGFloat = 260
}
