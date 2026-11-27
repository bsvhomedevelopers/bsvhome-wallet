import SwiftUI

public extension ButtonStylePalette {
    static let wowPayPrimary = ButtonStylePalette(
        foreground: Colors.WowPay.onBrand,
        foregroundPressed: Colors.WowPay.onBrand,
        background: Colors.WowPay.primaryButtonStart,
        backgroundPressed: Colors.WowPay.primaryButtonEnd,
        backgroundDisabled: Colors.WowPay.primaryButtonStart,
    )
    static let wowPaySecondary = ButtonStylePalette(
        foreground: Colors.WowPay.brand,
        foregroundPressed: Colors.WowPay.brand,
        background: Colors.WowPay.secondaryButton,
        backgroundPressed: Colors.WowPay.border,
        backgroundDisabled: Colors.WowPay.secondaryButton,
    )
}

public extension ButtonStyle where Self == ColorButtonStyle {
    static func wowPay() -> ColorButtonStyle {
        .styled(.wowPayPrimary, cornerRadius: Sizing.wowPay.controlCorner, glassEffect: .disabled, appearance: .wowPayPrimary)
    }

    static func wowPaySecondary() -> ColorButtonStyle {
        .styled(.wowPaySecondary, cornerRadius: Sizing.wowPay.controlCorner, glassEffect: .disabled, appearance: .wowPaySecondary)
    }
}
