// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeRuntimeEnvironment
import BsvHomeCorePrimitives
import GRDB
import Primitives

public struct ContactsRequest: DatabaseQueryable {
    public let chain: Chain?
    private let scopePolicy: AssetQueryScope

    public init(chain: Chain? = nil, environment: BsvHomeRuntimeEnvironment = .current) {
        self.chain = chain
        scopePolicy = AssetQueryScope(environment: environment)
    }

    public func fetch(_ db: Database) throws -> [ContactData] {
        var addresses = ContactRecord.addresses.filter(scopePolicy.chains.contains(ContactAddressRecord.Columns.chain))
        if let chain {
            addresses = addresses.filter(ContactAddressRecord.Columns.chain == chain.rawValue)
        }

        return try ContactRecord
            .including(all: addresses)
            .order(ContactRecord.Columns.name.asc)
            .asRequest(of: ContactRecordInfo.self)
            .fetchAll(db)
            .map(\.contactData)
    }
}
