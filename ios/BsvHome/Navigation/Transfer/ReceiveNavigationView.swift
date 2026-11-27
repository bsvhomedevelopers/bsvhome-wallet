import Primitives
import SwiftUI
import Transfer

struct ReceiveNavigationView: View {
    let model: ReceiveViewModel
    let navigationTitle: String

    init(model: ReceiveViewModel, wallet _: Wallet, navigationTitle: String = "") {
        self.model = model
        self.navigationTitle = navigationTitle
    }

    var body: some View {
        ReceiveScene(
            model: model,
            navigationTitle: navigationTitle,
        )
        .toolbar(.visible, for: .navigationBar)
    }
}
