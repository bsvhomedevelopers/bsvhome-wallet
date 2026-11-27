// Copyright (c). Gem Wallet. All rights reserved.

import class BsvHomeCore.BsvHomeAddressService
import struct BsvHomeCore.BsvHomeRecipient
import Components
import Foundation
import BsvHomeCorePrimitives
import Primitives

struct ContactRecipientSectionViewModel {
    private let contacts: [ContactData]

    init(contacts: [ContactData]) {
        self.contacts = contacts
    }

    var listItems: [ListItemValue<BsvHomeRecipient>] {
        contacts.flatMap { contactData in
            contactData.addresses.map { address in
                ListItemValue(
                    title: contactData.contact.name,
                    subtitle: BsvHomeAddressService.shared.format(address: address.address, chain: address.chain),
                    value: BsvHomeRecipient(address: address.address, name: contactData.contact.name, memo: address.memo),
                )
            }
        }
    }
}
