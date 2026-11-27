// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Localization
import PrimitivesComponents
import Style
import SwiftUI

public struct AboutUsScene: View {
    @State private var model: AboutUsViewModel

    public init(model: AboutUsViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        List {
            Section {
                SafariNavigationLink(url: model.termsOfServiceURL) {
                    ListItemView(title: model.termsOfServiceTitle)
                }
                SafariNavigationLink(url: model.privacyPolicyURL) {
                    ListItemView(title: model.privacyPolicyTitle)
                }
                SafariNavigationLink(url: model.websiteURL) {
                    ListItemView(title: model.websiteTitle)
                }
            }
            .listRowBackground(Colors.WowPay.surface)

            Section(model.communityTitle) {
                SocialLinksView(model: model.linksViewModel)
            }
            .listRowBackground(Colors.WowPay.surface)

            Section {
                ListItemView(
                    title: model.versionTextTitle,
                    subtitle: model.versionTextValue,
                )
                .contextMenu(model.contextMenuItems)

                if let version = model.releaseVersion {
                    NavigationCustomLink(
                        with: ListItemView(
                            title: Localized.UpdateApp.title,
                            subtitle: version,
                            imageStyle: .settings(assetImage: model.releaseImage),
                        ),
                        action: model.onUpdate,
                    )
                }
            }
            .listRowBackground(Colors.WowPay.surface)
        }
        .contentMargins(.top, .scene.top, for: .scrollContent)
        .listStyle(.insetGrouped)
        .listSectionSpacing(.compact)
        .scrollContentBackground(.hidden)
        .wowPayBackground()
        .navigationTitle(model.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .taskOnce { Task { await model.load() }}
    }
}
