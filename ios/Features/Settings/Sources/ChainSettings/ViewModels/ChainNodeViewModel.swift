// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Formatters
import struct BsvHomeCore.BsvHomeNodeSelection
import enum BsvHomeCore.BsvHomeNodeStatusState
import Localization
import Style

struct ChainNodeViewModel {
    let node: BsvHomeNodeSelection

    private let bsvHomeNodeFlag: String?
    private let statusState: BsvHomeNodeStatusState
    private let formatter: ValueFormatter

    init(
        node: BsvHomeNodeSelection,
        bsvHomeNodeFlag: String?,
        statusState: BsvHomeNodeStatusState,
        formatter: ValueFormatter,
    ) {
        self.node = node
        self.bsvHomeNodeFlag = bsvHomeNodeFlag
        self.statusState = statusState
        self.formatter = formatter
    }

    var url: String {
        node.url
    }

    var selection: String? {
        node.isSelected ? node.url : .none
    }

    var title: String {
        guard let bsvHomeNodeFlag else { return node.host }
        return Localized.Nodes.bsvHomeWalletNode + " " + bsvHomeNodeFlag
    }

    var titleExtra: String? {
        nodeStatusModel
            .latestBlockText(
                title: Localized.Nodes.ImportNode.latestBlock,
                formatter: formatter,
            )
    }

    var titleTag: String? {
        nodeStatusModel.latencyText
    }

    var titleTagType: TitleTagType {
        nodeStatusModel.titleTagType
    }

    var titleTagStyle: TextStyle {
        nodeStatusModel.titleTagStyle
    }

    private var nodeStatusModel: NodeStatusStateViewModel {
        NodeStatusStateViewModel(nodeStatus: statusState)
    }
}

// MARK: - Identifiable

extension ChainNodeViewModel: Identifiable {
    var id: String {
        node.url
    }
}
