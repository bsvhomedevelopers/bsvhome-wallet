import Observation
import SwiftUI

@Observable
@MainActor
final class BsvMultisigNavigation {
    let path: Binding<NavigationPath>

    init(path: Binding<NavigationPath>) {
        self.path = path
    }

    func append(_ route: BsvMultisigRoute) {
        path.wrappedValue.append(route)
    }

    func replaceLast(_ route: BsvMultisigRoute) {
        var updatedPath = path.wrappedValue
        if !updatedPath.isEmpty {
            updatedPath.removeLast()
        }
        updatedPath.append(route)
        path.wrappedValue = updatedPath
    }
}
