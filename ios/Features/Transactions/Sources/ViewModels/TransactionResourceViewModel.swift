// Copyright (c). Gem Wallet. All rights reserved.

import Components
import enum BsvHomeCore.Resource
import BsvHomeCorePrimitives
import Localization
import PrimitivesComponents

struct TransactionResourceViewModel {
    private let resource: BsvHomeCore.Resource?

    init(resource: BsvHomeCore.Resource?) {
        self.resource = resource
    }
}

extension TransactionResourceViewModel: ItemModelProvidable {
    var itemModel: TransactionItemModel {
        guard let resource else { return .empty }
        return .listItem(ListItemModel(title: Localized.Stake.resource, subtitle: ResourceViewModel(resource: resource.map()).title))
    }
}
