import Store
import StoreTestKit
import Testing

struct BsvMultisigStoreTests {
    @Test
    func compareAndSwapRejectsStaleSnapshot() throws {
        let store = BsvMultisigStore(db: .mock())
        #expect(try store.getState() == nil)
        #expect(try store.compareAndSwap(expectedRevision: 0, json: "first"))
        #expect(try !store.compareAndSwap(expectedRevision: 0, json: "stale"))
        #expect(try !store.compareAndSwap(expectedRevision: 2, json: "future"))
        #expect(try store.compareAndSwap(expectedRevision: 1, json: "second"))
        let state = try store.getState()
        let snapshot = try #require(state)
        #expect(snapshot.revision == 2)
        #expect(snapshot.json == "second")
    }

    @Test
    func stateSurvivesStoreRecreation() throws {
        let db = DB.mock()
        let store = BsvMultisigStore(db: db)
        #expect(try store.compareAndSwap(expectedRevision: 0, json: "authorization"))
        let recreated = BsvMultisigStore(db: db)
        #expect(try recreated.getState()?.json == "authorization")
        #expect(try recreated.getState()?.revision == 1)
    }

    @Test
    func overflowDoesNotOverwriteSnapshot() throws {
        let store = BsvMultisigStore(db: .mock())
        #expect(try store.compareAndSwap(expectedRevision: 0, json: "saved"))
        #expect(throws: (any Error).self) {
            try store.compareAndSwap(expectedRevision: UInt64.max, json: "invalid")
        }
        #expect(try store.getState()?.json == "saved")
    }
}
