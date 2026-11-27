// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Style
import SwiftUI

public struct SelectFilterView: View {
    private let style: ListItemView.Style
    private let typeModel: any FilterTypeRepresentable
    private let action: @MainActor @Sendable () -> Void

    public init(
        typeModel: any FilterTypeRepresentable,
        style: ListItemView.Style = .standard,
        action: @escaping @MainActor @Sendable () -> Void,
    ) {
        self.typeModel = typeModel
        self.style = style
        self.action = action
    }

    public var body: some View {
        NavigationCustomLink(
            with: ListItemView(model: ListItemModel(
                title: typeModel.title,
                subtitle: typeModel.value,
                imageStyle: .settings(assetImage: typeModel.image),
            ), style: style),
            action: action,
        )
    }
}

#Preview {
    SelectFilterView(
        typeModel: ChainsFilterTypeViewModel(type: .allChains),
        action: {},
    )
}
