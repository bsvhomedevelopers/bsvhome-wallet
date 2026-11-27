import Components
import SwiftUI

@MainActor
final class BsvColdWalletNavigation {
    private let owner: NavigationPathState
    private var path: Binding<NavigationPath> { owner.binding }
    private let workflowId = UUID()
    private var base: NavigationPath?
    private var rendered: NavigationPath?
    private var ownedPrefixes: [NavigationPath] = []
    private var didCloseInitialPage = false

    init(navigation: NavigationPathState) {
        owner = navigation
    }

    func pageRoute(_ page: BsvColdWalletRoute, at index: Int) -> BsvColdWalletPageRoute {
        BsvColdWalletPageRoute(workflowId: workflowId, index: index, page: page)
    }

    func owns(_ route: BsvColdWalletPageRoute, model: BsvColdWalletViewModel, navigation: NavigationPathState) -> Bool {
        guard owner === navigation, !model.isCancelled, route.workflowId == workflowId,
              model.path.indices.contains(route.index), model.path[route.index] == route.page,
              let base, let rendered, ownedPrefixes.contains(rendered)
        else { return false }
        var expected = base
        for (index, page) in model.path.prefix(route.index + 1).enumerated() {
            expected.append(pageRoute(page, at: index))
        }
        guard path.wrappedValue.count >= expected.count, ownedPrefixes.contains(expected) else { return false }
        var actual = path.wrappedValue
        actual.removeLast(actual.count - expected.count)
        return actual == expected
    }

    func show(_ pages: [BsvColdWalletRoute], model: BsvColdWalletViewModel) {
        if base == nil {
            base = path.wrappedValue
        }
        guard let base, path.wrappedValue.count >= base.count else { return }
        var currentPrefix = path.wrappedValue
        currentPrefix.removeLast(currentPrefix.count - base.count)
        guard currentPrefix == base else { return }
        if let rendered, path.wrappedValue != rendered, path.wrappedValue != base, !ownedPrefixes.contains(path.wrappedValue) {
            return
        }
        var next = base
        var prefixes = [base]
        for (index, page) in pages.enumerated() {
            next.append(pageRoute(page, at: index))
            prefixes.append(next)
        }
        ownedPrefixes = prefixes
        rendered = next
        guard path.wrappedValue != next else { return }
        model.projectNavigation(next, in: owner)
    }

    func appear(model: BsvColdWalletViewModel, onInitialPageBack: (@MainActor () -> Void)? = nil) {
        reconcile(model: model, onInitialPageBack: onInitialPageBack)
        guard !model.isCancelled else { return }
        model.attachNavigationRetirement { [weak self] in _ = self?.retire() }
        show(model.path, model: model)
    }

    @discardableResult
    func retire() -> Bool {
        guard let base, ownedPrefixes.contains(path.wrappedValue) else { return false }
        rendered = base
        if path.wrappedValue != base {
            path.wrappedValue = base
        }
        return path.wrappedValue == base
    }

    func isShowing(_ page: BsvColdWalletRoute, model: BsvColdWalletViewModel) -> Bool {
        guard !model.isCancelled, model.path.last == page, let base, let rendered, rendered == path.wrappedValue else { return false }
        var expected = base
        for (index, page) in model.path.enumerated() {
            expected.append(pageRoute(page, at: index))
        }
        return rendered == expected
    }

    func cancelInitialScanner(model: BsvColdWalletViewModel, onInitialPageBack: (@MainActor () -> Void)?) {
        guard !model.isCancelled, !didCloseInitialPage else { return }
        let retired = retire()
        model.cancelForNavigationRemoval()
        guard retired else { return }
        didCloseInitialPage = true
        onInitialPageBack?()
    }

    func reconcile(model: BsvColdWalletViewModel, onInitialPageBack: (@MainActor () -> Void)? = nil) {
        guard let base, rendered != path.wrappedValue else { return }
        let current = path.wrappedValue
        if let rendered, current.count > rendered.count {
            var prefix = current
            prefix.removeLast(current.count - rendered.count)
            if prefix == rendered {
                return
            }
        }
        guard current.count >= base.count else { model.cancel(); return }
        var prefix = current
        prefix.removeLast(current.count - base.count)
        guard prefix == base else { model.cancel(); return }
        guard ownedPrefixes.contains(current) else { return }
        rendered = current
        let depth = current.count - base.count
        if depth == 0, !model.isCancelled, !didCloseInitialPage,
           let initialRoute = model.initialRoute, model.path.first == initialRoute,
           let onInitialPageBack
        {
            didCloseInitialPage = true
            model.cancelForNavigationRemoval()
            onInitialPageBack()
            return
        }
        guard depth < model.path.count else { return }
        model.navigationPopped(to: depth)
    }
}
