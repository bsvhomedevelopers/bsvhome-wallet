// Copyright (c). Gem Wallet. All rights reserved.

import enum BsvHomeCore.BsvHomeNameRecordState
import struct BsvHomeCore.BsvHomeRecipient
import struct BsvHomeCore.BsvHomeRecipientValidation
import Components
import Foundation
import protocol BsvHomeCore.BsvHomeNameServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Primitives
import Style
import SwiftUI
import Validators

@Observable
@MainActor
public final class AddressInputViewModel {
    let placeholder: String
    public let nameRecordViewModel: NameRecordViewModel
    private let nameService: any BsvHomeNameServiceProtocol

    public var chain: Chain {
        didSet { onChangeChain() }
    }

    var inputModel: InputValidationViewModel

    public init(
        chain: Chain,
        nameService: any BsvHomeNameServiceProtocol,
        placeholder: String,
    ) {
        self.chain = chain
        self.placeholder = placeholder
        nameRecordViewModel = NameRecordViewModel(nameService: nameService)
        self.nameService = nameService
        inputModel = InputValidationViewModel(
            mode: .manual,
            validators: Self.validators(chain: chain, placeholder: placeholder, nameService: nameService),
        )
    }

    public var text: String {
        get { inputModel.text }
        set { inputModel.text = newValue }
    }

    public var nameResolveState: BsvHomeNameRecordState {
        nameRecordViewModel.state
    }

    public var isValid: Bool {
        validation.isValid
    }

    public var resolvedAddress: String {
        validation.address
    }

    public func recipient(memo: String?, references: [String] = []) throws -> BsvHomeRecipient {
        try nameService.recipient(
            chain: chain.rawValue,
            input: text,
            state: nameResolveState,
            memo: memo,
            references: references,
        )
    }

    private var validation: BsvHomeRecipientValidation {
        nameService.validateRecipient(chain: chain.rawValue, input: text, state: nameResolveState)
    }

    @discardableResult
    public func update() -> Bool {
        inputModel.update()
    }

    public func update(text: String) {
        inputModel.update(text: text)
    }

    public func update(error: (any Error)?) {
        inputModel.update(error: error)
    }

    @discardableResult
    public func validate() -> Bool {
        if nameRecordViewModel.isNameSupported(name: text) {
            isValid
        } else {
            update()
        }
    }

}

extension AddressInputViewModel {
    public var shouldShowInputActions: Bool {
        inputModel.text.isEmpty
    }

    func onSelectPaste() {
        guard let address = UIPasteboard.general.string else { return }
        update(text: address)
    }

    func onTextChange(_: String, newText: String) {
        nameRecordViewModel.getNameRecord(name: newText, chain: chain)
    }

    func onNameResolveStateChange(_: BsvHomeNameRecordState, newState: BsvHomeNameRecordState) {
        if newState.record() != nil {
            update(error: nil)
        }
    }
}

// MARK: - Private

extension AddressInputViewModel {
    private func onChangeChain() {
        nameRecordViewModel.reset()
        let currentText = text

        inputModel = InputValidationViewModel(
            mode: .manual,
            validators: Self.validators(chain: chain, placeholder: placeholder, nameService: nameService),
        )
        text = currentText

        if nameRecordViewModel.isNameSupported(name: currentText) {
            nameRecordViewModel.getNameRecord(name: currentText, chain: chain)
        } else if currentText.isNotEmpty {
            inputModel.update()
        }
    }

    private static func validators(chain: Chain, placeholder: String, nameService: any BsvHomeNameServiceProtocol) -> [any TextValidator] {
        [.required(requireName: placeholder), .address(chain.asset, nameService: nameService)]
    }
}
