// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeRecipient
import Components
import Foundation
import BsvHomeCorePrimitives
import Primitives

struct WalletRecipientSectionViewModel {
    private let wallets: [Wallet]
    private let section: RecipientAddressType
    private let chain: Chain

    init(wallets: [Wallet], section: RecipientAddressType, chain: Chain) {
        self.wallets = wallets
        self.section = section
        self.chain = chain
    }

    var listItems: [ListItemValue<BsvHomeRecipient>] {
        wallets
            .filter(walletFilter)
            .compactMap { wallet -> ListItemValue<BsvHomeRecipient>? in
                guard let account = wallet.accounts.first(where: { $0.chain == chain }) else {
                    return nil
                }
                return ListItemValue(
                    title: wallet.name,
                    subtitle: BsvHomeAddressService.shared.format(address: account.address, chain: account.chain),
                    value: BsvHomeRecipient(address: account.address, name: wallet.name),
                )
            }
    }

    private func walletFilter(_ wallet: Wallet) -> Bool {
        switch section {
        case .view: wallet.type == .view && !wallet.isPinned && wallet.accounts.first?.chain == chain
        case .wallets: (wallet.type == .multicoin || wallet.type == .single) && !wallet.isPinned && wallet.accounts.contains { $0.chain == chain }
        case .pinned: wallet.isPinned && wallet.accounts.contains { $0.chain == chain }
        case .contacts: false
        }
    }
}
