import SwiftUI

public extension TextStyle {
    enum wowPay {
        public static let rowTitle = TextStyle(font: .wowPay.rowTitle, color: Colors.WowPay.heading)
        public static let value = TextStyle(font: .wowPay.subtitle, color: Colors.WowPay.secondaryText)
        public static let caption = TextStyle(font: .wowPay.caption, color: Colors.WowPay.secondaryText)
    }

    func wowPay(font: WowPayFont, color: Color) -> TextStyle {
        let isNeutral = self.color == Colors.black || self.color == Colors.gray || self.color == Colors.secondaryText || self.color == .primary || self.color == .secondary
        return TextStyle(font: font, color: isNeutral ? color : self.color, fontWeight: fontWeight, background: background)
    }
}
