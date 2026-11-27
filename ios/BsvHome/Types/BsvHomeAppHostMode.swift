import Foundation

enum BsvHomeAppHostMode {
    static var isUnitTestHost: Bool {
        #if DEBUG
            ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
                || NSClassFromString("XCTestCase") != nil
        #else
            false
        #endif
    }
}
