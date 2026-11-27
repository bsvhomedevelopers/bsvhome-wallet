import GRDB

public struct FiatRatesRequest: DatabaseQueryable {
    public init() {}

    public func fetch(_ db: Database) throws -> [FiatRateRecord] {
        try FiatRateRecord.order(FiatRateRecord.Columns.symbol).fetchAll(db)
    }
}
