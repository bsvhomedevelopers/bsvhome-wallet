import SwiftUI

public enum WowPayFont: Sendable {
    case native(Font)
    case scaled(size: CGFloat, weight: Font.Weight, relativeTo: Font.TextStyle)

    public func weight(_ weight: Font.Weight) -> WowPayFont {
        switch self {
        case let .native(font):
            .native(font.weight(weight))
        case let .scaled(size, _, style):
            .scaled(size: size, weight: weight, relativeTo: style)
        }
    }

    fileprivate struct Modifier: ViewModifier {
        let font: WowPayFont?
        @ScaledMetric private var size: CGFloat

        init(font: WowPayFont?) {
            self.font = font
            switch font {
            case let .some(.scaled(size, _, style)):
                _size = ScaledMetric(wrappedValue: size, relativeTo: style)
            case .some(.native), .none:
                _size = ScaledMetric(wrappedValue: 0)
            }
        }

        func body(content: Content) -> some View {
            switch font {
            case let .some(.native(value)):
                content.font(value)
            case let .some(.scaled(_, weight, _)):
                content.font(.system(size: size, weight: weight))
            case .none:
                content.font(nil as Font?)
            }
        }
    }
}

public extension View {
    func font(_ font: WowPayFont?) -> some View {
        modifier(WowPayFont.Modifier(font: font))
    }
}
