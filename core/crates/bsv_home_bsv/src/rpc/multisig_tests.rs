use std::sync::Arc;

use bitcoin::{Transaction, consensus::deserialize};
use bsv_home_client::{ClientError, testkit::MockClient};
use serde::Deserialize;

use super::ScriptTransactionHistory;
use crate::multisig::{PreviousOutput, merge_signatures, parse_locking_script, plan_transfer, sign_plan};

use super::{BsvClient, BsvRequestScheduler};

const SCRIPT: &str = "5221024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d07662102531fe6068134503d2723133227c867ac8fa6c83c537e9a44c3c5bdbdcb1fe33721031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f53ae";
const SCRIPT_HASH: &str = "be171d693230f5e5a85e2ff58b92b0729dafb4768e567ae38a9e91c9849837a3";

fn provider(client: MockClient) -> BsvClient<MockClient> {
    BsvClient::with_scheduler(client, Arc::new(BsvRequestScheduler::default()))
}

#[tokio::test]
async fn test_unused_script_history_accepts_confirmed_first_page_not_found() {
    let client = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/confirmed/history?limit=1000") {
            Err(ClientError::Http {
                status: 404,
                body: b"Not Found".to_vec(),
            })
        } else if path == format!("/script/{SCRIPT_HASH}/unconfirmed/history?limit=1000") {
            Ok(format!("{{\"script\":\"{SCRIPT_HASH}\",\"result\":[],\"error\":\"\"}}").into_bytes())
        } else {
            Err(ClientError::Http { status: 400, body: vec![] })
        }
    }));
    let configuration = parse_locking_script(SCRIPT).unwrap();
    assert!(client.script_transaction_history(&configuration).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_empty_confirmed_history_still_reads_unconfirmed_transactions() {
    let client = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/confirmed/history?limit=1000") {
            Err(ClientError::Http {
                status: 404,
                body: b"Not Found".to_vec(),
            })
        } else {
            Ok(format!("{{\"script\":\"{SCRIPT_HASH}\",\"result\":[{{\"tx_hash\":\"{}\"}}],\"error\":\"\"}}", "01".repeat(32)).into_bytes())
        }
    }));
    let history = client.script_history(SCRIPT).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].tx_hash, "01".repeat(32));
    assert_eq!(history[0].height, None);
}

#[tokio::test]
async fn test_script_history_does_not_treat_other_errors_as_empty() {
    for error in [
        ClientError::Http { status: 404, body: vec![] },
        ClientError::Http {
            status: 404,
            body: b"route unavailable".to_vec(),
        },
        ClientError::Http {
            status: 500,
            body: b"Not Found".to_vec(),
        },
        ClientError::Serialization("invalid JSON".into()),
    ] {
        let client = provider(MockClient::new().with_get(move |_| Err(error.clone())));
        assert!(client.script_history(SCRIPT).await.is_err());
    }
    let client = provider(MockClient::new().with_get(|_| {
        Err(ClientError::Http {
            status: 404,
            body: b"Not Found".to_vec(),
        })
    }));
    assert!(client.script_history(SCRIPT).await.is_err());
    assert!(client.script_unspent(SCRIPT).await.is_err());
    let wrong_owner = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/confirmed/history?limit=1000") {
            Err(ClientError::Http {
                status: 404,
                body: b"Not Found".to_vec(),
            })
        } else {
            Ok(format!("{{\"script\":\"{}\",\"result\":[],\"error\":\"\"}}", "01".repeat(32)).into_bytes())
        }
    }));
    assert!(wrong_owner.script_history(SCRIPT).await.is_err());
}

#[tokio::test]
async fn test_script_history_does_not_hide_later_page_not_found() {
    let client = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/confirmed/history?limit=1000") {
            Ok(format!(
                "{{\"script\":\"{SCRIPT_HASH}\",\"result\":[{{\"tx_hash\":\"{}\",\"height\":10}}],\"error\":\"\",\"nextPageToken\":\"next\"}}",
                "01".repeat(32)
            )
            .into_bytes())
        } else {
            Err(ClientError::Http {
                status: 404,
                body: b"Not Found".to_vec(),
            })
        }
    }));
    assert!(client.script_history(SCRIPT).await.is_err());
}

#[tokio::test]
async fn test_script_unspent_follows_pages_and_propagates_partial_failure() {
    let client = provider(MockClient::new().with_get(|path| {
        let first = format!("/script/{SCRIPT_HASH}/unspent/all");
        let second = format!("{first}?token=next+%2F%2B");
        match path {
            path if path == first => Ok(include_bytes!("../../testdata/multisig_unspent_page1.json").to_vec()),
            path if path == second => Ok(include_bytes!("../../testdata/multisig_unspent_page2.json").to_vec()),
            _ => Err(ClientError::Http { status: 400, body: vec![] }),
        }
    }));
    let outputs = client.script_unspent(SCRIPT).await.unwrap();
    assert_eq!(
        outputs.iter().map(|output| (output.tx_hash.clone(), output.value)).collect::<Vec<_>>(),
        [("01".repeat(32), 10_000), ("02".repeat(32), 10_000)]
    );
    let failed = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/unspent/all") {
            Ok(include_bytes!("../../testdata/multisig_unspent_page1.json").to_vec())
        } else {
            Err(ClientError::Http { status: 400, body: vec![] })
        }
    }));
    assert!(failed.script_unspent(SCRIPT).await.is_err());
}

#[tokio::test]
async fn test_script_response_rejects_owner_error_status_and_duplicates() {
    for changed in [
        include_str!("../../testdata/multisig_unspent_page2.json").replace(SCRIPT_HASH, &"03".repeat(32)),
        include_str!("../../testdata/multisig_unspent_page2.json").replace("\"error\": \"\"", "\"error\": \"failed\""),
        include_str!("../../testdata/multisig_unspent_page2.json").replace("confirmed", "unknown"),
    ] {
        let client = provider(MockClient::new().with_get(move |_| Ok(changed.as_bytes().to_vec())));
        assert!(client.script_unspent(SCRIPT).await.is_err());
    }
    let client = provider(MockClient::new().with_get(|path| {
        if path == format!("/script/{SCRIPT_HASH}/unspent/all") {
            Ok(include_bytes!("../../testdata/multisig_unspent_page1.json").to_vec())
        } else {
            Ok(include_str!("../../testdata/multisig_unspent_page2.json")
                .replace(&"02".repeat(32), &"01".repeat(32))
                .as_bytes()
                .to_vec())
        }
    }));
    assert!(client.script_unspent(SCRIPT).await.is_err());
}

#[tokio::test]
async fn test_raw_transaction_recomputes_identity() {
    let raw = "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1c03d7c6082f7376706f6f6c2e636f6d2f3edff034600055b8467f0040ffffffff01247e814a000000001976a914492558fb8ca71a3591316d095afc0f20ef7d42f788ac00000000";
    let client = provider(MockClient::new().with_get(move |_| Ok(raw.as_bytes().to_vec())));
    assert_eq!(
        client.raw_transaction("c1d32f28baa27a376ba977f6a8de6ce0a87041157cef0274b20bfda2b0d8df96").await.unwrap(),
        raw
    );
    assert!(client.raw_transaction(&"01".repeat(32)).await.is_err());
}

#[tokio::test]
async fn test_spent_output_distinguishes_unspent_from_unknown() {
    let unspent = provider(MockClient::new().with_get(|_| Err(ClientError::Http { status: 404, body: vec![] })));
    assert_eq!(unspent.outpoint_spend(&"01".repeat(32), 0).await.unwrap(), None);
    let unknown = provider(MockClient::new().with_get(|_| Err(ClientError::Http { status: 400, body: vec![] })));
    assert!(unknown.outpoint_spend(&"01".repeat(32), 0).await.is_err());
}

#[derive(Deserialize)]
struct SourceVectors {
    vectors: Vec<SourceVector>,
}

#[derive(Deserialize)]
struct SourceVector {
    previous: String,
}

#[tokio::test]
async fn test_script_previous_outputs_require_confirmed_unspent_and_raw_evidence() {
    let fixture: SourceVectors = serde_json::from_str(include_str!("../../testdata/multisig_sdk_vectors.json")).unwrap();
    let raw = fixture.vectors[1].previous.clone();
    let client = provider(MockClient::new().with_get(move |path| {
        if path.ends_with("/unspent/all") {
            Ok(include_bytes!("../../testdata/multisig_confirmed_selection.json").to_vec())
        } else if path.ends_with("/hex") {
            Ok(raw.as_bytes().to_vec())
        } else {
            Ok(include_bytes!("../../testdata/multisig_previous_info.json").to_vec())
        }
    }));
    let configuration = parse_locking_script(SCRIPT).unwrap();
    let previous = client.script_previous_outputs(&configuration).await.unwrap();
    assert_eq!(previous.iter().map(|source| (source.vout, source.value)).collect::<Vec<_>>(), [(0, 10_000)]);
    for altered in [
        include_str!("../../testdata/multisig_previous_info.json").replace("\"confirmations\": 10", "\"confirmations\": 0"),
        include_str!("../../testdata/multisig_previous_info.json").replace("\"blockheight\": 10", "\"blockheight\": 11"),
    ] {
        let raw = fixture.vectors[1].previous.clone();
        let client = provider(MockClient::new().with_get(move |path| {
            if path.ends_with("/unspent/all") {
                Ok(include_bytes!("../../testdata/multisig_confirmed_selection.json").to_vec())
            } else if path.ends_with("/hex") {
                Ok(raw.as_bytes().to_vec())
            } else {
                Ok(altered.as_bytes().to_vec())
            }
        }));
        assert!(client.script_previous_outputs(&configuration).await.is_err());
    }
}

#[tokio::test]
async fn test_script_balance_validates_confirmed_and_pending_evidence() {
    let fixture: SourceVectors = serde_json::from_str(include_str!("../../testdata/multisig_sdk_vectors.json")).unwrap();
    let raw = fixture.vectors[1].previous.clone();
    let client = provider(MockClient::new().with_get(move |path| {
        if path.ends_with("/unspent/all") {
            Ok(include_bytes!("../../testdata/multisig_balance.json").to_vec())
        } else if path.contains("3e9a237e5784d2b1ea340778b1acb3971a165d96bfa1327e8fcdbb930a64ce39") {
            if path.ends_with("/hex") {
                Ok(format!("02000000{}", &raw[8..]).as_bytes().to_vec())
            } else {
                Ok(include_bytes!("../../testdata/multisig_unconfirmed_info.json").to_vec())
            }
        } else if path.ends_with("/hex") {
            Ok(raw.as_bytes().to_vec())
        } else {
            Ok(include_bytes!("../../testdata/multisig_previous_info.json").to_vec())
        }
    }));
    let configuration = parse_locking_script(SCRIPT).unwrap();
    assert_eq!(client.script_balance(&configuration).await.unwrap(), (10_000, 10_000));
    let raw = fixture.vectors[1].previous.clone();
    let mismatched = provider(MockClient::new().with_get(move |path| {
        if path.ends_with("/unspent/all") {
            Ok(include_str!("../../testdata/multisig_balance.json")
                .replace("\"value\": 10000", "\"value\": 10001")
                .as_bytes()
                .to_vec())
        } else if path.ends_with("/hex") {
            Ok(raw.as_bytes().to_vec())
        } else {
            Ok(include_bytes!("../../testdata/multisig_previous_info.json").to_vec())
        }
    }));
    assert!(mismatched.script_balance(&configuration).await.is_err());
}

#[tokio::test]
async fn test_script_transaction_history_proves_incoming_and_outgoing_ownership() {
    let fixture: SourceVectors = serde_json::from_str(include_str!("../../testdata/multisig_sdk_vectors.json")).unwrap();
    let source_raw = fixture.vectors[1].previous.clone();
    let source: Transaction = deserialize(&hex::decode(&source_raw).unwrap()).unwrap();
    let configuration = parse_locking_script(SCRIPT).unwrap();
    let previous = source
        .output
        .iter()
        .enumerate()
        .map(|(index, output)| PreviousOutput {
            transaction_id: source.compute_txid().to_string(),
            vout: index as u32,
            value: output.value.to_sat(),
            raw_transaction: source_raw.clone(),
        })
        .collect::<Vec<_>>();
    let plan = plan_transfer(&configuration, &previous, "76a91479b000887626b294a914501a4cd226b58b23598388ac", None, 100).unwrap();
    let finalized = merge_signatures(
        &configuration,
        &plan,
        &[sign_plan(&configuration, &plan, &[1; 32]).unwrap(), sign_plan(&configuration, &plan, &[2; 32]).unwrap()],
    )
    .unwrap();
    let incoming = source.compute_txid().to_string();
    let outgoing = finalized.transaction_id.clone();
    let expected = vec![
        ScriptTransactionHistory {
            transaction_id: incoming.clone(),
            confirmations: 10,
        },
        ScriptTransactionHistory {
            transaction_id: outgoing.clone(),
            confirmations: 10,
        },
    ];
    let client = provider(MockClient::new().with_get(move |path| {
        if path.starts_with("/script/") {
            if path.contains("/unconfirmed/") {
                return Ok(format!("{{\"script\":\"{SCRIPT_HASH}\",\"result\":[],\"error\":\"\"}}").into_bytes());
            }
            return Ok(format!(
                "{{\"script\":\"{SCRIPT_HASH}\",\"result\":[{{\"tx_hash\":\"{incoming}\",\"height\":10}},{{\"tx_hash\":\"{outgoing}\",\"height\":10}}],\"error\":\"\"}}"
            )
            .into_bytes());
        }
        if path == format!("/tx/{incoming}/hex") {
            return Ok(source_raw.as_bytes().to_vec());
        }
        if path == format!("/tx/{outgoing}/hex") {
            return Ok(finalized.raw_transaction.as_bytes().to_vec());
        }
        for transaction_id in [&incoming, &outgoing] {
            if path == format!("/tx/hash/{transaction_id}") {
                return Ok(format!("{{\"txid\":\"{transaction_id}\",\"confirmations\":10,\"blockheight\":10,\"vin\":[],\"vout\":[]}}").into_bytes());
            }
        }
        Err(ClientError::Http { status: 400, body: vec![] })
    }));
    assert_eq!(client.script_transaction_history(&configuration).await.unwrap(), expected);
}
