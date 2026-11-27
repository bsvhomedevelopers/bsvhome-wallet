import SwiftUI

public enum WowPayGradients {
    public static let balance = LinearGradient(
        stops: [
            .init(color: Colors.WowPay.balanceStart, location: 0),
            .init(color: Colors.WowPay.balanceMiddle, location: 0.48),
            .init(color: Colors.WowPay.balanceEnd, location: 1),
        ],
        startPoint: .topLeading,
        endPoint: .bottomTrailing,
    )
    public static let action = LinearGradient(
        colors: [Colors.WowPay.actionStart, Colors.WowPay.actionEnd],
        startPoint: .topLeading,
        endPoint: .bottomTrailing,
    )
    public static let primaryButton = LinearGradient(
        colors: [Colors.WowPay.primaryButtonStart, Colors.WowPay.primaryButtonEnd],
        startPoint: .topLeading,
        endPoint: .bottomTrailing,
    )
    public static let page = RadialGradient(
        colors: [Colors.WowPay.backgroundAccent, Colors.WowPay.background],
        center: .topTrailing,
        startRadius: 0,
        endRadius: 360,
    )
}
