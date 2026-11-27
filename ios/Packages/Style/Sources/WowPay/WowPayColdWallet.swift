import SwiftUI

public extension Sizing.wowPay {
    static let coldWalletQR: CGFloat = 260
    static let coldWalletServiceIcon: CGFloat = 22
    static let coldWalletServiceAvatar: CGFloat = 42
}

public extension Spacing.wowPay {
    static let coldWalletPage: CGFloat = 14
    static let coldWalletPageBottom: CGFloat = 28
    static let coldWalletServiceVertical: CGFloat = 14
}

public extension WowPayFont.wowPay {
    static let coldWalletValue: WowPayFont = .scaled(size: 15, weight: .regular, relativeTo: .subheadline)
    static let coldWalletServiceSubtitle: WowPayFont = .scaled(size: 10, weight: .regular, relativeTo: .caption2)
    static let coldWalletUnsupportedTitle: WowPayFont = .scaled(size: 24, weight: .regular, relativeTo: .title2)
    static let coldWalletDescription: WowPayFont = .scaled(size: 16, weight: .regular, relativeTo: .body)
}
