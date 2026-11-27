// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import enum BsvHomeCore.BsvHomeServiceError
@testable import BsvHomeCorePrimitives
import Testing

struct ErrorCancellationTests {
    @Test func isCancelled() {
        #expect(BsvHomeServiceError.Cancelled.isCancelled)
        #expect(CancellationError().isCancelled)
        #expect(NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled).isCancelled)

        #expect(!BsvHomeServiceError.Api(msg: "failed").isCancelled)
        #expect(!NSError(domain: NSURLErrorDomain, code: NSURLErrorTimedOut).isCancelled)
    }
}
