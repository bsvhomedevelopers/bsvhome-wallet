// Copyright (c). Gem Wallet. All rights reserved.

import Components
@testable import Contacts
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Primitives
import PrimitivesTestKit
import Testing

@MainActor
struct ManageContactAddressViewModelTests {
    private let bsvAddress = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"

    @Test
    func buttonStateAddMode() {
        let model = ManageContactAddressViewModel.mock(mode: .add)

        #expect(model.buttonState == .disabled)

        model.onSelectChain(.bitcoinSv)
        model.addressInputModel.update(text: bsvAddress)

        #expect(model.buttonState == .normal)
    }

    @Test
    func buttonStateEditMode() {
        let model = ManageContactAddressViewModel.mock(mode: .edit(.mock(address: bsvAddress, chain: .bitcoinSv)))

        #expect(model.buttonState == .normal)

        model.addressInputModel.update(text: "")

        #expect(model.buttonState == .disabled)
    }

    @Test
    func networkChoicesMatchTheProductScope() {
        let model = ManageContactAddressViewModel.mock(mode: .add)

        #expect(model.chain == .bitcoinSv)
        #expect(model.networkSelectorModel.items == [.bitcoinSv])
        model.onSelectChain(.bitcoinSv)
        #expect(model.showMemo == false)
        model.memo = "retained"
        model.onSelectChain(.cosmos)
        #expect(model.chain == .bitcoinSv)
        #expect(model.memo == "retained")
    }

    @Test
    func nameResolveState() {
        let model = ManageContactAddressViewModel.mock(mode: .add)
        model.addressInputModel.text = "john"

        model.addressInputModel.nameRecordViewModel.state = .loading(name: "john")
        #expect(model.buttonState == .disabled)

        model.addressInputModel.nameRecordViewModel.state = .error
        #expect(model.buttonState == .disabled)

        model.onSelectChain(.bitcoinSv)
        model.addressInputModel.nameRecordViewModel.state = .complete(record: NameRecord.mock(name: "john", chain: .bitcoinSv, address: bsvAddress).map())
        let resolvedState = model.addressInputModel.nameRecordViewModel.state
        #expect(model.buttonState == .normal)

        model.onSelectChain(.tron)
        #expect(model.chain == .bitcoinSv)
        #expect(model.addressInputModel.nameRecordViewModel.state == resolvedState)
        #expect(model.buttonState == .normal)
    }

    @Test
    func retainedHiddenAddressCannotBeSubmitted() {
        var completions = 0
        let address = ContactAddress.mock()
        let model = ManageContactAddressViewModel(
            service: BsvHomeManageContactServiceMock(environment: .wowPay),
            nameService: BsvHomeNameServiceMock(),
            mode: .edit(address),
            onComplete: { _ in completions += 1 },
        )

        #expect(model.chain == address.chain)
        #expect(model.addressInputModel.text == address.address)
        #expect(model.buttonState == .disabled)
        model.complete()
        #expect(completions == 0)
    }
}

// MARK: - Mock

extension ManageContactAddressViewModel {
    static func mock(mode: Mode) -> ManageContactAddressViewModel {
        ManageContactAddressViewModel(
            service: BsvHomeManageContactServiceMock(environment: .wowPay),
            nameService: BsvHomeNameServiceMock(),
            mode: mode,
            onComplete: { _ in },
        )
    }
}
