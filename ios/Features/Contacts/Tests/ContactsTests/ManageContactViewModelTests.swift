// Copyright (c). Gem Wallet. All rights reserved.

@testable import Contacts
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import Testing

@MainActor
struct ManageContactViewModelTests {
    @Test
    func visibleDeletionPreservesHiddenStoredAddresses() {
        let hidden = ContactAddress.mock(id: "hidden", chain: .ethereum)
        let bsv = ContactAddress.mock(id: "bsv", chain: .bitcoinSv)
        let tron = ContactAddress.mock(id: "tron", chain: .tron)
        let model = ManageContactViewModel.mock(mode: .edit(.mock(addresses: [hidden, bsv, tron])))

        #expect(model.visibleAddresses == [bsv])
        #expect(model.addresses == [hidden, bsv, tron])
        model.deleteAddress(at: [0])
        #expect(model.addresses == [hidden, tron])
        #expect(model.visibleAddresses.isEmpty)
    }

    @Test
    func hiddenNetworkCannotCreateANewAddress() {
        let model = ManageContactViewModel.mock(mode: .add())
        model.onAddressComplete(.init(chain: .ethereum, address: "0x0000000000000000000000000000000000000001", memo: nil, replacingId: nil))
        #expect(model.addresses.isEmpty)
    }

    @Test
    func buttonStateAddMode() {
        let model = ManageContactViewModel.mock(mode: .add())

        #expect(model.buttonState == .disabled)

        model.nameInputModel.text = "John"

        #expect(model.buttonState == .normal)
    }

    @Test
    func buttonStateEditMode() {
        let model = ManageContactViewModel.mock(mode: .edit(.mock(contact: .mock(name: "John"), addresses: [.mock()])))

        #expect(model.buttonState == .normal)

        model.nameInputModel.text = ""

        #expect(model.buttonState == .disabled)
    }
}

// MARK: - Mock

extension ManageContactViewModel {
    static func mock(mode: Mode) -> ManageContactViewModel {
        ManageContactViewModel(service: BsvHomeManageContactServiceMock(environment: .wowPay), nameService: BsvHomeNameServiceMock(), mode: mode)
    }
}
