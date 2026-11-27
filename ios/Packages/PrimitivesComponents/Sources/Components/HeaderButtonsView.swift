// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Localization
import Primitives
import Style
import SwiftUI

public typealias HeaderButtonAction = @MainActor @Sendable (HeaderButtonType) -> Void

public struct HeaderButtonsView: View {
    public enum Style {
        case standard
        case compactMenu
        case menuItems
        case wowPay
        case wowPayTokenActions
    }

    private let style: Style
    private let buttons: [HeaderButton]
    private var action: HeaderButtonAction?

    var maxWidth: CGFloat {
        buttons.count > 3 ? 84 : 94
    }

    public init(
        buttons: [HeaderButton],
        style: Style = .standard,
        action: HeaderButtonAction? = nil,
    ) {
        self.buttons = buttons
        self.style = style
        self.action = action
    }

    public var body: some View {
        switch style {
        case .standard:
            HStack(alignment: .center, spacing: 2) {
                ForEach(buttons) {
                    buttonView(for: $0)
                }
            }
        case .wowPay:
            LazyVGrid(
                columns: [GridItem(.flexible(), spacing: Spacing.wowPay.grid), GridItem(.flexible(), spacing: Spacing.wowPay.grid)],
                spacing: Spacing.wowPay.grid,
            ) {
                ForEach(buttons) { button in
                    wowPayButton(for: button)
                        .buttonStyle(button.type == .send ? ColorButtonStyle.wowPay() : .wowPaySecondary())
                        .disabled(!button.isEnabled)
                        .accessibilityIdentifier(button.id)
                }
            }
        case .wowPayTokenActions:
            LazyVGrid(
                columns: Array(repeating: GridItem(.flexible(), spacing: Spacing.wowPay.grid), count: 3),
                spacing: Spacing.wowPay.grid,
            ) {
                ForEach(buttons) { button in
                    tokenAction(for: button)
                        .buttonStyle(.plain)
                        .disabled(!button.isEnabled)
                        .opacity(button.isEnabled ? 1 : WowPayOpacity.disabled)
                        .accessibilityIdentifier(button.id)
                }
            }
        case .compactMenu:
            Menu {
                ForEach(buttons) { button in
                    menuItem(for: button)
                        .disabled(!button.isEnabled)
                        .accessibilityIdentifier(button.id)
                }
            } label: {
                Label {
                    Text(Localized.Wallet.more)
                } icon: {
                    Images.Actions.more
                        .renderingMode(.template)
                }
                .font(.wowPay.label)
                .foregroundStyle(Colors.WowPay.brand)
                .frame(minHeight: Sizing.wowPay.minimumTapTarget)
            }
            .disabled(buttons.isEmpty)
        case .menuItems:
            ForEach(buttons) { button in
                menuItem(for: button)
                    .disabled(!button.isEnabled)
                    .accessibilityIdentifier(button.id)
            }
        }
    }

    @ViewBuilder
    private func wowPayButton(for button: HeaderButton) -> some View {
        switch button.viewType {
        case .button:
            Button(button.title) {
                action?(button.type)
            }
        case let .menuButton(title, items):
            Menu {
                ForEach(items) { ActionMenuItemView(item: $0) }
            } label: {
                Text(title ?? button.title)
            }
        }
    }

    @ViewBuilder
    private func tokenAction(for button: HeaderButton) -> some View {
        switch button.viewType {
        case .button:
            Button {
                action?(button.type)
            } label: {
                tokenActionLabel(title: button.title, image: tokenActionImage(for: button))
            }
        case let .menuButton(title, items):
            Menu {
                ForEach(items) { ActionMenuItemView(item: $0) }
            } label: {
                tokenActionLabel(title: title ?? button.title, image: tokenActionImage(for: button))
            }
        }
    }

    private func tokenActionLabel(title: String, image: Image) -> some View {
        VStack(spacing: Spacing.space8) {
            image
                .renderingMode(.template)
                .resizable()
                .scaledToFit()
                .frame(width: Sizing.wowPay.tokenActionIcon, height: Sizing.wowPay.tokenActionIcon)
                .foregroundStyle(Colors.WowPay.brand)
                .accessibilityHidden(true)
            Text(title)
                .font(.wowPay.rowTitle)
                .foregroundStyle(Colors.WowPay.heading)
                .multilineTextAlignment(.center)
        }
        .padding(.vertical, Spacing.wowPay.servicePadding)
        .padding(.horizontal, Spacing.space8)
        .frame(maxWidth: .infinity, minHeight: Sizing.wowPay.minimumTapTarget)
        .wowPayCard(padding: .zero, cornerRadius: Sizing.wowPay.serviceCorner)
        .contentShape(RoundedRectangle(cornerRadius: Sizing.wowPay.serviceCorner))
    }

    private func tokenActionImage(for button: HeaderButton) -> Image {
        switch button.type {
        case .send: Image("wowpay-send")
        case .receive: Image("wowpay-receive")
        case .swap: Image("wowpay-swap")
        default: button.image
        }
    }

    @ViewBuilder
    private func menuItem(for button: HeaderButton) -> some View {
        switch button.viewType {
        case .button:
            Button {
                action?(button.type)
            } label: {
                Label { Text(button.title) } icon: { button.image }
            }
        case let .menuButton(title, items):
            Menu {
                ForEach(items) { ActionMenuItemView(item: $0) }
            } label: {
                Label { Text(title ?? button.title) } icon: { button.image }
            }
        }
    }

    private func buttonView(for button: HeaderButton) -> some View {
        Group {
            switch button.viewType {
            case .button:
                RoundButton(
                    title: button.title,
                    image: button.image,
                    isEnabled: button.isEnabled,
                ) {
                    action?(button.type)
                }
            case let .menuButton(title, items):
                AdaptiveActionMenu(
                    title: title,
                    items: items,
                    label: {
                        RoundButton(
                            title: button.title,
                            image: button.image,
                            isEnabled: button.isEnabled,
                            action: {}, // action empty, handled by menu
                        )
                    },
                )
            }
        }
        .accessibilityIdentifier(button.id)
        .frame(maxWidth: maxWidth, alignment: .center)
    }
}

// MARK: - Previews

#Preview {
    let buttons = HeaderButtonType.allCases.map { HeaderButton(type: $0, isEnabled: true) }
    return VStack {
        Spacer()
        HeaderButtonsView(buttons: buttons, action: nil)
        Spacer()
    }
}
