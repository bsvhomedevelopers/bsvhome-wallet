// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

public struct StateButtonStyle: ButtonStyle {
    public static let maxHeight: CGFloat = 50
    @Environment(\.isEnabled) private var isEnabled
    private let variant: ButtonType
    private let palette: ButtonStylePalette

    public init(_ variant: ButtonType, palette: ButtonStylePalette) {
        self.variant = variant
        self.palette = palette
    }

    @ViewBuilder
    public func makeBody(configuration: Configuration) -> some View {
        if !usesGlass {
            Group {
                if variant.state.showProgress {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .tint(Colors.whiteSolid)
                        .frame(maxWidth: .infinity, minHeight: buttonHeight)
                } else {
                    configuration.label
                        .fontWeight(.bold)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                        .foregroundStyle(foreground(configuration: configuration))
                        .padding(.horizontal, .medium)
                        .padding(.vertical, .space12)
                        .frame(maxWidth: .infinity, minHeight: buttonHeight)
                }
            }
            .background {
                RoundedRectangle(cornerRadius: cornerRadius)
                    .fill(background(configuration: configuration))
            }
            .opacity(shouldDim ? WowPayOpacity.disabled : 1)
        } else {
            standardBody(configuration: configuration)
        }
    }

    private func standardBody(configuration: Configuration) -> some View {
        ZStack {
            adoptiveShape(configuration: configuration)

            if variant.state.showProgress {
                ProgressView()
                    .progressViewStyle(.circular)
                    .tint(Colors.whiteSolid)
            } else {
                let label = configuration.label
                    .lineLimit(1)
                    .foregroundStyle(foreground(configuration: configuration))
                    .padding(.horizontal, .medium)
                    .frame(maxWidth: .infinity, maxHeight: buttonHeight)
                label
            }
        }
        .frame(height: usesGlass ? nil : buttonHeight)
        .opacity(shouldDim ? WowPayOpacity.disabled : 1)
    }

    @ViewBuilder
    private func adoptiveShape(configuration: Configuration) -> some View {
        if #available(iOS 26, *), usesGlass {
            DefaultGlassEffectShape()
                .fill(background(configuration: configuration))
                .frame(maxHeight: buttonHeight)
                .glassEffect(.regular.interactive(!variant.isDisabled))
        } else {
            RoundedRectangle(cornerRadius: cornerRadius)
                .fill(background(configuration: configuration))
                .frame(maxHeight: buttonHeight)
        }
    }

    private var usesGlass: Bool {
        switch variant {
        case .primary: true
        case .wowPayPrimary: false
        }
    }

    private var cornerRadius: CGFloat {
        switch variant {
        case .primary: Sizing.space12
        case .wowPayPrimary: Sizing.wowPay.controlCorner
        }
    }

    private var buttonHeight: CGFloat {
        usesGlass ? Self.maxHeight : Sizing.wowPay.buttonHeight
    }

    private var shouldDim: Bool {
        guard !usesGlass else { return false }
        switch variant.state {
        case .normal: return !isEnabled
        case let .loading(show): return !show
        case .disabled: return true
        }
    }

    private func background(configuration: Configuration) -> AnyShapeStyle {
        if case .wowPayPrimary = variant, !configuration.isPressed {
            return AnyShapeStyle(WowPayGradients.primaryButton)
        }
        let color = switch variant.state {
        case .normal: configuration.isPressed ? palette.backgroundPressed : palette.background
        case let .loading(show): show ? palette.background : palette.backgroundDisabled
        case .disabled: palette.backgroundDisabled
        }
        return AnyShapeStyle(color)
    }

    private func foreground(configuration: Configuration) -> Color {
        switch variant.state {
        case .normal: configuration.isPressed ? palette.foregroundPressed : palette.foreground
        case let .loading(show): show ? palette.foreground : palette.foreground.opacity(.semi)
        case .disabled: palette.foreground
        }
    }
}

// MARK: - ButtonStyle Static

public extension ButtonStyle where Self == StateButtonStyle {
    static func primary(_ state: ButtonState = .normal) -> Self {
        .init(.primary(state), palette: .primary)
    }

    static func variant(_ variant: ButtonType) -> Self {
        switch variant {
        case let .primary(state): .primary(state)
        case .wowPayPrimary: .init(variant, palette: .wowPayPrimary)
        }
    }
}

// MARK: - Previews

#Preview {
    List {
        Section("Helpers .primary") {
            Button("Primary · normal") {}
                .buttonStyle(.primary())
            Button("Primary · loading") {}
                .buttonStyle(.primary(.loading()))
            Button("Primary · disabled") {}
                .buttonStyle(.primary(.disabled))
        }
    }
    .padding()
}
