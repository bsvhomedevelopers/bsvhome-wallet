// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

public struct ColorButtonStyle: ButtonStyle {
    public enum Appearance {
        case standard
        case wowPayPrimary
        case wowPaySecondary
    }

    @Environment(\.isEnabled) private var isEnabled
    @ScaledMetric(relativeTo: .callout) private var wowPayLabelSize = WowPayTypography.rowTitleSize

    let palette: ButtonStylePalette
    let paddingHorizontal: CGFloat
    let paddingVertical: CGFloat
    let cornerRadius: CGFloat
    let glassEffect: GlassEffectSettings
    let appearance: Appearance

    public init(
        palette: ButtonStylePalette,
        paddingHorizontal: CGFloat,
        paddingVertical: CGFloat,
        cornerRadius: CGFloat,
        glassEffect: GlassEffectSettings,
        appearance: ColorButtonStyle.Appearance = .standard,
    ) {
        self.paddingHorizontal = paddingHorizontal
        self.paddingVertical = paddingVertical
        self.palette = palette
        self.cornerRadius = cornerRadius
        self.glassEffect = glassEffect
        self.appearance = appearance
    }

    @ViewBuilder
    public func makeBody(configuration: Configuration) -> some View {
        let content = configuration.label
            .frame(minWidth: 0, maxWidth: .infinity)
            .padding(.vertical, paddingVertical)
            .padding(.horizontal, paddingHorizontal)
            .frame(minHeight: appearance == .standard ? nil : Sizing.wowPay.buttonHeight)
            .foregroundStyle(foregroundStyle(configuration: configuration))
            .background(background(configuration: configuration))
        if appearance == .standard {
            content.fontWeight(.semibold)
        } else {
            content
                .font(.system(size: wowPayLabelSize, weight: .bold))
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .opacity(isEnabled ? 1 : WowPayOpacity.disabled)
        }
    }

    @ViewBuilder
    private func background(configuration: Configuration) -> some View {
        if #available(iOS 26, *), glassEffect.isEnabled {
            DefaultGlassEffectShape()
                .fill(configuration.isPressed ? palette.backgroundPressed : palette.background)
                .glassEffect(.regular.interactive(glassEffect.isInteractive))
        } else {
            RoundedRectangle(cornerRadius: cornerRadius)
                .fill(backgroundStyle(configuration: configuration))
        }
    }

    private func backgroundStyle(configuration: Configuration) -> AnyShapeStyle {
        if appearance == .wowPayPrimary, !configuration.isPressed {
            return AnyShapeStyle(WowPayGradients.primaryButton)
        }
        return AnyShapeStyle(configuration.isPressed ? palette.backgroundPressed : palette.background)
    }

    private func foregroundStyle(configuration: Configuration) -> some ShapeStyle {
        configuration.isPressed ? palette.foregroundPressed : palette.foreground
    }
}

// MARK: - ButtonStyle Static

public extension ButtonStyle where Self == ColorButtonStyle {
    static func styled(
        _ palette: ButtonStylePalette,
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
        appearance: ColorButtonStyle.Appearance = .standard,
    ) -> ColorButtonStyle {
        ColorButtonStyle(
            palette: palette,
            paddingHorizontal: paddingHorizontal,
            paddingVertical: paddingVertical,
            cornerRadius: cornerRadius,
            glassEffect: glassEffect,
            appearance: appearance,
        )
    }

    static func blue(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.blue, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func blueGrayPressed(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.blueGrayPressed, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func gray(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.gray, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func lightGray(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.lightGray, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func white(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.white, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func empty(
        paddingHorizontal: CGFloat = .space12,
        paddingVertical: CGFloat = .small,
        cornerRadius: CGFloat = .small,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.empty, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func amount(
        paddingHorizontal: CGFloat = .small,
        paddingVertical: CGFloat = .small,
        cornerRadius: CGFloat = .small,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.amount, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func listStyleColor(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.listStyleColor, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func red(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.red, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func green(
        paddingHorizontal: CGFloat = .button.paddingHorizontal,
        paddingVertical: CGFloat = .button.paddingVertical,
        cornerRadius: CGFloat = Sizing.space12,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.green, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }

    static func listEmpty(
        paddingHorizontal: CGFloat = .small,
        paddingVertical: CGFloat = .small,
        cornerRadius: CGFloat = .small,
        glassEffect: GlassEffectSettings = .isInteractive,
    ) -> ColorButtonStyle {
        .styled(.listEmpty, paddingHorizontal: paddingHorizontal, paddingVertical: paddingVertical, cornerRadius: cornerRadius, glassEffect: glassEffect)
    }
}

// MARK: – Previews

#Preview {
    List {
        Section("ColorButtonStyle presets") {
            Button("Blue") {}
                .buttonStyle(.blue())
            Button("Blue Gray Pressed") {}
                .buttonStyle(.blueGrayPressed())
            Button("Gray") {}
                .buttonStyle(.gray())
            Button("Light Gray") {}
                .buttonStyle(.lightGray())
            Button("List Style Color") {}
                .buttonStyle(.listStyleColor())
            Button("White") {}
                .buttonStyle(.white())
            Button("Empty") {}
                .buttonStyle(.empty())
            Button("Amount") {}
                .buttonStyle(.amount())
        }
    }
    .padding()
}
