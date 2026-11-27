// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import GRDB
import os
import Primitives

public struct DB: Sendable {
    private static let ignoreMethods = ["COMMIT TRANSACTION", "PRAGMA query_only", "BEGIN DEFERRED TRANSACTION"].asSet()
    public let dbQueue: DatabaseQueue

    public init(
        fileName: String = "db.sqlite",
        configuration: GRDB.Configuration = DB.defaultConfiguration,
    ) {
        let queue: DatabaseQueue
        do {
            // TODO: - remove the logic FileMigrator in 2026
            let fileMigrator = FileMigrator()
            let databaseURL = try fileMigrator.migrate(
                name: fileName,
                fromDirectory: .documentDirectory,
                toDirectory: .applicationSupportDirectory,
                isDirectory: false,
            )
            queue = try Self.open(fileURL: databaseURL, configuration: configuration)
        } catch {
            os_log(
                "DB Initialization error: %{public}@",
                log: .default,
                type: .fault,
                error.localizedDescription,
            )
            fatalError("DB Initialization error: \(error)")
        }

        do {
            try self.init(dbQueue: queue)
        } catch {
            os_log(
                "DB Migration error: %{public}@",
                log: .default,
                type: .fault,
                error.localizedDescription,
            )
            fatalError("DB Migration error: \(error)")
        }
    }

    public init(fileURL: URL, configuration: GRDB.Configuration = DB.defaultConfiguration) throws {
        try self.init(dbQueue: Self.open(fileURL: fileURL, configuration: configuration))
    }

    private init(dbQueue: DatabaseQueue) throws {
        self.dbQueue = dbQueue
        var migrations = Migrations()
        try migrations.run(dbQueue: dbQueue)
        if !isRunningTests {
            try migrations.runChanges(dbQueue: dbQueue)
        }
    }

    private static func open(fileURL: URL, configuration: GRDB.Configuration) throws -> DatabaseQueue {
        try DatabaseQueue(path: fileURL.path(percentEncoded: false), configuration: configuration)
    }

    public static let defaultConfiguration: GRDB.Configuration = {
        var config = GRDB.Configuration()
        #if DEBUG
            config.publicStatementArguments = true
            config.prepareDatabase { db in
                db.trace { // sql in
                    switch $0 {
                    case let .profile(statement, duration):
                        break
                    // debugLog("profile SQL> \(statement)")
                    case let .statement(statement):
                        let sql = statement.sql

                        if ignoreMethods.filter({ sql.description.contains($0) }).isEmpty {
                            // debugLog("SQL> \(sql)")
                        }
                    }
                }
            }
        #endif
        return config
    }()
}

var isRunningTests: Bool {
    ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
}
