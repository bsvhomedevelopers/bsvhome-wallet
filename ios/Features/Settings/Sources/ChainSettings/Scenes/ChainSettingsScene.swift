// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import Localization
import PrimitivesComponents
import Style
import SwiftUI

public struct ChainSettingsScene: View {
    @State private var model: ChainSettingsSceneViewModel

    public init(model: ChainSettingsSceneViewModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        List {
            if let error = model.errorMessage {
                Section {
                    StateEmptyView(title: error) {
                        Button(Localized.Common.tryAgain, action: model.onRetry)
                            .disabled(!model.canRetry)
                    }
                    .accessibilityIdentifier("chainSettingsError")
                }
            }
            if model.isLoading {
                ProgressView().frame(maxWidth: .infinity)
                    .accessibilityIdentifier("chainSettingsLoading")
            }
            Section(model.nodesTitle) {
                ForEach(model.nodesModels) { nodeModel in
                    ListItemSelectionView(
                        title: nodeModel.title,
                        titleExtra: nodeModel.titleExtra,
                        titleTag: nodeModel.titleTag,
                        titleTagType: nodeModel.titleTagType,
                        titleTagStyle: nodeModel.titleTagStyle,
                        subtitle: .none,
                        subtitleExtra: .none,
                        value: nodeModel.url,
                        selection: nodeModel.selection,
                        action: model.onSelectNode,
                    )
                    .contextMenu(
                        .copy(value: nodeModel.url),
                    )
                    .if(model.canDelete(url: nodeModel.url)) {
                        $0.swipeActions(edge: .trailing) {
                            Button(model.deleteButtonTitle, role: .destructive) {
                                model.onSelectNodeForDeletion(nodeModel.node)
                            }
                            .tint(Colors.red)
                        }
                    }
                }
            }
            .disabled(model.isLoading)

            Section(model.explorerTitle) {
                ForEach(model.explorers, id: \.self) { explorer in
                    ListItemSelectionView(
                        title: explorer,
                        titleExtra: .none,
                        titleTag: .none,
                        titleTagType: .none,
                        subtitle: .none,
                        subtitleExtra: .none,
                        value: explorer,
                        selection: model.selectedExplorer,
                        action: model.onSelectExplorer(name:),
                    )
                }
            }
            .disabled(model.isLoading)
        }
        .accessibilityIdentifier("settingsChainDetail")
        .refreshable {
            await model.load()
        }
        .alert(
            model.deleteConfirmationTitle(for: model.nodeDelete?.host ?? ""),
            presenting: $model.nodeDelete,
            sensoryFeedback: .warning,
            actions: { _ in
                Button(
                    model.deleteButtonTitle,
                    role: .destructive,
                    action: model.onDeleteNode,
                )
            },
        )
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: model.onPresentImportNode) {
                    Images.System.plus
                        .font(.body.weight(.semibold))
                }
                .accessibilityIdentifier("settingsAddNode")
                .disabled(model.isLoading)
            }
        }
        .sheet(item: importNodeBinding) { imported in
            NavigationStack {
                AddNodeScene(model: imported, onDismiss: { model.onDismissImportNode(id: imported.id) })
            }
        }
        .navigationTitle(model.title)
        .listSectionSpacing(.compact)
        .taskOnce {
            Task { await model.load() }
        }
    }

    private var importNodeBinding: Binding<AddNodeSceneViewModel?> {
        let id = model.presentedImportNode?.id
        return Binding(get: { model.presentedImportNode }, set: { replacement in
            guard replacement == nil, let id else { return }
            model.onDismissImportNode(id: id)
        })
    }
}
