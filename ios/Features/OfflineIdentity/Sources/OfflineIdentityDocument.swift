import Foundation
import SwiftUI
import UniformTypeIdentifiers

struct OfflineIdentityDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.json, .plainText] }
    let text: String

    init(text: String) {
        self.text = text
    }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents, data.count <= 16384, let text = String(data: data, encoding: .utf8) else { throw OfflineIdentityError.invalidDocument }
        self.text = text
    }

    func fileWrapper(configuration _: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}
