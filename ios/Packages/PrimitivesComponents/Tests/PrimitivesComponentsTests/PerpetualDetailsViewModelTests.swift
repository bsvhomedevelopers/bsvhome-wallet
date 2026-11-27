// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import func BsvHomeCore.perpetualDetails
import struct BsvHomeCore.PerpetualConfirmData
import enum BsvHomeCore.PerpetualDirection
import struct BsvHomeCore.PerpetualReduceData
import enum BsvHomeCore.PerpetualType
import BsvHomeCorePrimitivesTestKit
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
import Testing

struct PerpetualDetailsViewModelTests {
    @Test
    func leverageText() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(leverage: 5))).leverageText == "5x")
    }

    @Test
    func slippageField() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(slippage: 2.0))).slippageField.value.text == "2.0000%")
    }

    @Test
    func entryPriceField() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(entryPrice: 48000.0))).entryPriceField?.value.text == "$48,000.0000")
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(entryPrice: nil))).entryPriceField == nil)
    }

    @Test
    func marginField() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(marginAmount: 1000.0))).marginField.value.text == "$1,000.0000")
    }

    @Test
    func sizeField() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(fiatValue: 5000.0))).sizeField.value.text == "$5,000.0000")
    }

    @Test
    func positionText() {
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(direction: .long, leverage: 40))).positionText == "Long 40x")
        #expect(PerpetualDetailsViewModel.mock(.open(.mock(direction: .short, leverage: 10))).positionText == "Short 10x")
        #expect(PerpetualDetailsViewModel.mock(.increase(.mock(direction: .long, leverage: 5))).positionText == "Long 5x")

        let reduceModel = PerpetualDetailsViewModel.mock(.reduce(positionDirection: .short))
        #expect(reduceModel.positionText == "Short 3x")
    }

    @Test
    func listItemModelSubtitle() {
        let closeModel = PerpetualDetailsViewModel.mock(.close(.mock(pnl: 500, marginAmount: 1000)))
        #expect(closeModel.listItemModel.title == "Details")
        #expect(closeModel.listItemModel.subtitle == "+$500.0000 (+50.0000%)")

        #expect(PerpetualDetailsViewModel.mock(.open(.mock())).listItemModel.subtitle == "Long 3x")

        let increaseModel = PerpetualDetailsViewModel.mock(.increase(.mock()))
        #expect(increaseModel.listItemModel.subtitle == "Increase Long")

        let increaseShortModel = PerpetualDetailsViewModel.mock(.increase(.mock(direction: .short)))
        #expect(increaseShortModel.listItemModel.subtitle == "Increase Short")

        let reduceLongModel = PerpetualDetailsViewModel.mock(.reduce())
        #expect(reduceLongModel.listItemModel.subtitle == "Reduce Long")

        let reduceShortModel = PerpetualDetailsViewModel.mock(.reduce(positionDirection: .short))
        #expect(reduceShortModel.listItemModel.subtitle == "Reduce Short")
    }
}

extension PerpetualDetailsViewModel {
    static func mock(_ type: PerpetualType) -> PerpetualDetailsViewModel {
        PerpetualDetailsViewModel(details: perpetualDetails(perpetualType: type)!)
    }
}

extension PerpetualType {
    static func open(_ data: PerpetualConfirmData = .mock()) -> PerpetualType {
        .open(data: data)
    }

    static func close(_ data: PerpetualConfirmData = .mock()) -> PerpetualType {
        .close(data: data)
    }

    static func increase(_ data: PerpetualConfirmData = .mock()) -> PerpetualType {
        .increase(data: data)
    }

    static func reduce(
        _ data: PerpetualConfirmData = .mock(),
        positionDirection: PerpetualDirection = .long,
    ) -> PerpetualType {
        .reduce(data: PerpetualReduceData(data: data, positionDirection: positionDirection))
    }
}
