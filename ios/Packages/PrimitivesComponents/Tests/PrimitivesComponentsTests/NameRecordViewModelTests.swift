// Copyright (c). Gem Wallet. All rights reserved.

import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import Primitives
@testable import PrimitivesComponents
import PrimitivesTestKit
import Testing

@MainActor
struct NameRecordViewModelTests {
    @Test
    func keepsTheResolvedStateAndSkipsARepeatedName() async {
        let record = NameRecord.mock()
        let nameService = BsvHomeNameServiceMock(nameRecord: record)
        let model = NameRecordViewModel(nameService: nameService)

        model.getNameRecord(name: record.name, chain: record.chain)
        await model.nameRecordTask?.value

        #expect(model.state == .complete(record: record.map()))

        model.getNameRecord(name: record.name, chain: record.chain)

        #expect(model.state == .complete(record: record.map()))
        #expect(nameService.requestedNames == [record.name])
    }

    @Test
    func failedLookupEndsInError() async {
        let model = NameRecordViewModel(nameService: BsvHomeNameServiceMock(error: AnyError("offline")))

        model.getNameRecord(name: "test.eth", chain: .ethereum)
        await model.nameRecordTask?.value

        #expect(model.state == .error)
    }
}
