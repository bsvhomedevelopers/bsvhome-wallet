// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import enum BsvHomeCore.BsvHomeAddressFormatStyle
import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeRecipient
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style

public struct AddressListItemViewModel {
    public enum Mode {
        case auto(addressStyle: BsvHomeAddressFormatStyle)
        case address(addressStyle: BsvHomeAddressFormatStyle)
        case nameOrAddress
        case nameAndAddress
    }

    public let title: String
    public let account: SimpleAccount
    public let mode: Mode
    public let onAddContact: ((AddContactType) -> Void)?
    private let addressLink: BlockExplorerLink

    public init(
        title: String,
        account: SimpleAccount,
        mode: Mode,
        addressLink: BlockExplorerLink,
        onAddContact: ((AddContactType) -> Void)? = nil,
    ) {
        self.title = title
        self.account = account
        self.mode = mode
        self.addressLink = addressLink
        self.onAddContact = onAddContact
    }

    public var subtitle: String {
        switch mode {
        case let .auto(style): auto(for: style)
        case let .address(style): address(for: style)
        case .nameOrAddress, .nameAndAddress: account.name ?? account.address
        }
    }

    public var subtitleExtra: String? {
        guard case .nameAndAddress = mode, account.name?.isNotEmpty == true else { return nil }
        return account.address
    }

    public var assetImage: AssetImage? {
        account.assetImage
    }

    public var addressExplorerText: String {
        Localized.Transaction.viewOn(addressLink.name)
    }

    public var addressExplorerUrl: URL {
        addressLink.url
    }

    public var createContactTitle: String {
        Localized.Contacts.createNewContact
    }

    public var createContactImage: String {
        SystemImage.personBadgePlus
    }

    public var addToExistingContactTitle: String {
        Localized.Contacts.addToExistingContact
    }

    public var addToExistingContactImage: String {
        SystemImage.personCircle
    }

    public var addContactRecipient: BsvHomeRecipient {
        BsvHomeRecipient(address: account.address, memo: account.memo)
    }

    public var canToggleAddress: Bool {
        if case .nameAndAddress = mode {
            return false
        }
        guard let name = account.name, name.isNotEmpty else {
            return false
        }
        return name != account.address
    }

    public var addressSubtitle: String {
        address(for: .short)
    }

    // MARK: - Private methods

    private func auto(for style: BsvHomeAddressFormatStyle) -> String {
        if account.name == account.address || account.name == nil {
            return address(for: style)
        } else if let _ = account.assetImage, let name = account.name {
            return name
        } else if let name = account.name {
            let address = address(for: .short)
            if address.isEmpty {
                return name
            }
            return "\(name) (\(address))"
        }
        return account.address
    }

    private func address(for style: BsvHomeAddressFormatStyle) -> String {
        BsvHomeAddressService.shared.format(address: account.address, chain: account.chain, style: style)
    }
}
