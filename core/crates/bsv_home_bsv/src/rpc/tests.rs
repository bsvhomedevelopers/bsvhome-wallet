use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use bitcoin::{Address as BitcoinAddress, Network, PubkeyHash, hashes::Hash};
use bsv_home_client::{ClientError, testkit::MockClient};
use chain_traits::{ChainBalances, ChainState, ChainTransaction, ChainTransactionBroadcast, ChainTransactionLoad};
use futures::{channel::oneshot, join};
use primitives::{Address, Asset, BroadcastOptions, Chain, TransactionIdRequest, TransactionInputType};

use super::{
    BsvClient, BsvRequestScheduler,
    target::{BsvTarget, Confirmation},
};

fn provider(client: MockClient) -> BsvClient<MockClient> {
    BsvClient::with_scheduler(client, Arc::new(BsvRequestScheduler::default()))
}

fn history_addresses(count: u8) -> Vec<String> {
    (0..count)
        .map(|index| BitcoinAddress::p2pkh(PubkeyHash::from_byte_array([index; 20]), Network::Bitcoin).to_string())
        .collect()
}

fn empty_histories(addresses: &[String]) -> Vec<serde_json::Value> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/wallet_history.json")).unwrap();
    addresses
        .iter()
        .map(|address| {
            let mut response = fixture[0].clone();
            response["address"] = address.clone().into();
            for section in ["confirmed", "unconfirmed"] {
                response[section]["result"] = Vec::<serde_json::Value>::new().into();
            }
            response
        })
        .collect()
}

#[tokio::test]
async fn test_bulk_histories_chunk_at_twenty_and_restore_input_order() {
    let addresses = history_addresses(41);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let calls = requests.clone();
    let client = provider(MockClient::new().with_post(move |path, body| {
        assert_eq!(path, "/addresses/history/all");
        let body: serde_json::Value = serde_json::from_slice(body).unwrap();
        let addresses: Vec<String> = serde_json::from_value(body["addresses"].clone()).unwrap();
        calls.lock().unwrap().push(addresses.clone());
        let mut response = empty_histories(&addresses);
        response.reverse();
        Ok(serde_json::to_vec(&response).unwrap())
    }));
    let histories = client.histories(&addresses).await.unwrap();
    assert_eq!(histories.iter().map(|(address, _)| address).collect::<Vec<_>>(), addresses.iter().collect::<Vec<_>>());
    assert_eq!(histories.iter().map(|(_, history)| history.len()).collect::<Vec<_>>(), vec![0; 41]);
    assert_eq!(requests.lock().unwrap().iter().map(Vec::len).collect::<Vec<_>>(), [20, 20, 1]);
}

#[tokio::test]
async fn test_bulk_histories_reject_invalid_inputs_before_requests() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }));
    let addresses = history_addresses(21);
    let mut malformed = addresses.clone();
    malformed.push("invalid-address".to_string());
    let mut duplicate = addresses.clone();
    duplicate.push(addresses[0].clone());
    for addresses in [Vec::new(), malformed, duplicate] {
        assert!(client.histories(&addresses).await.is_err());
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_bulk_histories_reject_missing_duplicate_unrequested_and_failed_sections() {
    let addresses = history_addresses(2);
    let valid = empty_histories(&addresses);
    let mut cases = vec![vec![valid[0].clone()], vec![valid[0].clone(), valid[0].clone()]];
    let mut unrequested = valid.clone();
    unrequested[0]["address"] = history_addresses(3)[2].clone().into();
    cases.push(unrequested);
    let mut extra = valid.clone();
    extra.push(empty_histories(&history_addresses(3))[2].clone());
    cases.push(extra);
    for section in ["confirmed", "unconfirmed"] {
        let mut response = valid.clone();
        response[1][section]["error"] = "supplier failed".into();
        cases.push(response);
        for field in ["error", "result"] {
            let mut response = valid.clone();
            response[1][section].as_object_mut().unwrap().remove(field);
            cases.push(response);
        }
    }
    for response in cases {
        let client = provider(MockClient::new().with_post(move |_, _| Ok(serde_json::to_vec(&response).unwrap())));
        assert!(client.histories(&addresses).await.is_err());
    }
}

#[tokio::test]
async fn test_bulk_histories_propagate_failed_later_batches() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let client = provider(MockClient::new().with_post(move |_, body| {
        if calls.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(ClientError::Http { status: 403, body: Vec::new() });
        }
        let body: serde_json::Value = serde_json::from_slice(body).unwrap();
        let addresses: Vec<String> = serde_json::from_value(body["addresses"].clone()).unwrap();
        Ok(serde_json::to_vec(&empty_histories(&addresses)).unwrap())
    }));
    assert_eq!(client.histories(&history_addresses(21)).await.unwrap_err().to_string(), "HTTP error: status 403");
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_bulk_histories_bind_each_continuation_to_its_address() {
    let addresses = history_addresses(2);
    let mut response = empty_histories(&addresses);
    response[0]["confirmed"]["nextPageToken"] = "next".into();
    let mut page = response[1]["confirmed"].clone();
    page["address"] = addresses[1].clone().into();
    let client = provider(
        MockClient::new()
            .with_post(move |_, _| Ok(serde_json::to_vec(&response).unwrap()))
            .with_get(move |_| Ok(serde_json::to_vec(&page).unwrap())),
    );
    assert_eq!(client.histories(&addresses).await.unwrap_err().to_string(), "invalid WhatsOnChain address response");
}

#[tokio::test]
async fn empty_balance_uses_combined_unspent_endpoint() {
    let client = provider(MockClient::new().with_get(|path| {
        assert_eq!(path, "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/unspent/all");
        Ok(br#"{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","error":"","result":[]}"#.to_vec())
    }));
    let balance = client.get_balance_coin("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string()).await.unwrap();
    assert_eq!(balance.balance.available.to_string(), "0");
    assert_eq!(balance.balance.pending_unconfirmed.to_string(), "0");
}

#[tokio::test]
async fn native_balance_refresh_has_no_staking_balance_or_request() {
    let client = provider(MockClient::new());
    assert_eq!(client.get_balance_staking("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string()).await.unwrap(), None);
}

#[tokio::test]
async fn balance_rejects_provider_errors_missing_status_and_duplicate_outputs() {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let output = serde_json::json!({"tx_hash":format!("{:064}",1),"tx_pos":0,"value":1,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"});
    let mut missing = output.clone();
    missing.as_object_mut().unwrap().remove("status");
    let mut invalid = output.clone();
    invalid["status"] = serde_json::json!("unknown");
    let mut immature = output.clone();
    immature["height"] = serde_json::json!(0);
    for results in [
        serde_json::json!([missing]),
        serde_json::json!([invalid]),
        serde_json::json!([immature]),
        serde_json::json!([output.clone(), output]),
    ] {
        let client = provider(MockClient::new().with_get(move |_| Ok(serde_json::to_vec(&serde_json::json!({"address":address,"error":"","result":results})).unwrap())));
        assert!(client.get_balance_coin(address.to_string()).await.is_err());
    }
    for response in [
        Err(ClientError::Http { status: 404, body: vec![] }),
        Ok(br#"{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","error":"supplier failed","result":[]}"#.to_vec()),
        Ok(br#"{"address":"other","error":"","result":[]}"#.to_vec()),
    ] {
        let client = provider(MockClient::new().with_get(move |_| response.clone()));
        assert!(client.get_balance_coin(address.to_string()).await.is_err());
    }
}

#[tokio::test]
async fn combined_unspent_follows_its_own_pagination_token() {
    let client = provider(MockClient::new().with_get(|path| {
        let (value, token) = match path {
            "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/unspent/all" => (1, Some("next /+")),
            "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/unspent/all?token=next+%2F%2B" => (2, None),
            _ => panic!("unexpected UTXO path: {path}"),
        };
        Ok(serde_json::to_vec(&serde_json::json!({"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","error":"","nextPageToken":token,"result":[{"tx_hash":format!("{value:064}"),"tx_pos":0,"value":50000,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"}]})).unwrap())
    }));
    let outputs = client.unspent("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap();
    assert_eq!(outputs.len(), 2);
    assert_ne!(outputs[0].tx_hash, outputs[1].tx_hash);
}

#[tokio::test]
async fn history_uses_bulk_post_and_independent_section_tokens() {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let client = provider(
        MockClient::new()
            .with_post(move |path, body| {
                assert_eq!(path, "/addresses/history/all");
                assert_eq!(serde_json::from_slice::<serde_json::Value>(body).unwrap(), serde_json::json!({"addresses":[address]}));
                Ok(serde_json::to_vec(&serde_json::json!([{"address":address,"confirmed":{"error":"","result":[{"tx_hash":"confirmed-first","height":10}],"nextPageToken":"confirmed +"},"unconfirmed":{"error":"","result":[{"tx_hash":"pending-first"}],"nextPageToken":"pending /"}}])).unwrap())
            })
            .with_get(move |path| {
                let (txid, height) = match path {
                    "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/unconfirmed/history?limit=1000&token=pending+%2F" => ("pending-next", None),
                    "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/confirmed/history?limit=1000&token=confirmed+%2B" => ("confirmed-next", Some(10)),
                    _ => panic!("unexpected history path: {path}"),
                };
                Ok(serde_json::to_vec(&serde_json::json!({"address":address,"error":"","result":[{"tx_hash":txid,"height":height}]})).unwrap())
            }),
    );
    let history = client.history(address).await.unwrap();
    assert_eq!(
        history.iter().map(|entry| entry.tx_hash.as_str()).collect::<Vec<_>>(),
        ["pending-first", "pending-next", "confirmed-first", "confirmed-next"]
    );
}

#[tokio::test]
async fn empty_bulk_history_does_not_request_legacy_first_pages() {
    let client = provider(MockClient::new().with_post(|path, _| {
        assert_eq!(path, "/addresses/history/all");
        Ok(br#"[{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","confirmed":{"result":[],"error":""},"unconfirmed":{"result":[],"error":""}}]"#.to_vec())
    }));
    assert_eq!(client.history("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap().len(), 0);
}

#[tokio::test]
async fn bulk_history_retries_read_only_posts_but_not_permanent_failures() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(ClientError::Http { status: 429, body: vec![] });
        }
        Ok(br#"[{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","confirmed":{"result":[],"error":""},"unconfirmed":{"result":[],"error":""}}]"#.to_vec())
    }));
    assert_eq!(client.history("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap().len(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        count.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::Http { status: 404, body: vec![] })
    }));
    assert_eq!(
        client.history("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap_err().to_string(),
        "HTTP error: status 404"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejects_incomplete_bulk_history_and_section_errors() {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let valid = serde_json::json!({"address":address,"confirmed":{"result":[],"error":""},"unconfirmed":{"result":[],"error":""}});
    let mut cases = vec![serde_json::json!([]), serde_json::json!([valid.clone(), valid.clone()])];
    for field in ["address", "confirmed", "unconfirmed"] {
        let mut response = valid.clone();
        response.as_object_mut().unwrap().remove(field);
        cases.push(serde_json::json!([response]));
    }
    for section in ["confirmed", "unconfirmed"] {
        for field in ["result", "error"] {
            let mut response = valid.clone();
            response[section].as_object_mut().unwrap().remove(field);
            cases.push(serde_json::json!([response]));
        }
        let mut response = valid.clone();
        response[section]["error"] = serde_json::json!("supplier failed");
        cases.push(serde_json::json!([response]));
    }
    let mut response = valid;
    response["address"] = serde_json::json!("wrong-address");
    cases.push(serde_json::json!([response]));
    for response in cases {
        let client = provider(MockClient::new().with_post(move |_, _| Ok(serde_json::to_vec(&response).unwrap())));
        assert!(client.history(address).await.is_err());
    }
}

#[tokio::test]
async fn bulk_history_rejects_repeated_tokens_and_failed_continuations() {
    for status in [200, 404] {
        let client = provider(
            MockClient::new()
                .with_post(|_, _| {
                    Ok(
                        br#"[{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","confirmed":{"result":[],"error":"","nextPageToken":"next"},"unconfirmed":{"result":[],"error":""}}]"#
                            .to_vec(),
                    )
                })
                .with_get(move |_| match status {
                    200 => Ok(br#"{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","result":[],"error":"","nextPageToken":"next"}"#.to_vec()),
                    _ => Err(ClientError::Http { status, body: vec![] }),
                }),
        );
        let expected = if status == 200 { "BSV pagination token repeated" } else { "HTTP error: status 404" };
        assert_eq!(client.history("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap_err().to_string(), expected);
    }
}

#[tokio::test]
async fn bulk_history_first_page_counts_towards_pagination_limit() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let client = provider(
        MockClient::new()
            .with_post(|_, _| {
                Ok(
                    br#"[{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","confirmed":{"result":[],"error":"","nextPageToken":"first"},"unconfirmed":{"result":[],"error":""}}]"#
                        .to_vec(),
                )
            })
            .with_get(move |_| {
                let token = count.fetch_add(1, Ordering::SeqCst).to_string();
                Ok(serde_json::to_vec(&serde_json::json!({"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","result":[],"error":"","nextPageToken":token})).unwrap())
            }),
    );
    assert_eq!(
        client.history("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").await.unwrap_err().to_string(),
        "BSV pagination exceeded 100 pages"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 99);
}
#[tokio::test]
async fn throttles_and_retries_rate_limited_reads() {
    let requests = Arc::new(AtomicUsize::new(0));
    let calls = requests.clone();
    let client = provider(MockClient::new().with_get(move |_| {
        if calls.fetch_add(1, Ordering::SeqCst) < 2 {
            return Err(ClientError::Http { status: 429, body: vec![] });
        }
        Ok(br#"{"chain":"main","blocks":968714,"headers":968714}"#.to_vec())
    }));
    let start = std::time::Instant::now();
    assert_eq!(client.get_block_latest_number().await.unwrap(), 968714);
    assert_eq!(requests.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= std::time::Duration::from_secs(3));
}
#[tokio::test]
async fn does_not_retry_permanent_errors_or_accept_fractional_fee_quotes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let client = provider(MockClient::new().with_get(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::Http { status: 400, body: vec![] })
    }));
    assert!(client.get_block_latest_number().await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let input_type = TransactionInputType::Transfer {
        asset: Asset::from_chain(Chain::BitcoinSv),
    };
    for data in [
        r#"{"fee_unit":"sat/KB","fee":100.01,"mempool_min_fee":100}"#,
        r#"{"fee_unit":"sat/B","fee":100,"mempool_min_fee":100}"#,
    ] {
        let data = data.as_bytes().to_vec();
        let client = provider(MockClient::new().with_get(move |_| Ok(data.clone())));
        assert!(client.get_transaction_fee_rates(input_type.clone()).await.is_err());
    }
}
#[tokio::test]
async fn excludes_mempool_spent_outputs_and_tracks_unconfirmed_balance() {
    let client = provider(MockClient::new().with_get(|path| {
        let txid = |value: u8| format!("{value:064}");
        if path.starts_with("/tx/hash/") {
            let id = path.strip_prefix("/tx/hash/").unwrap();
            let confirmed = id == txid(1);
            let script = hex::encode(crate::BsvAddress::from_str("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").unwrap().script_pubkey().as_bytes());
            return Ok(serde_json::to_vec(&serde_json::json!({"txid":id,"confirmations":if confirmed {1} else {0},"vin":[{"txid":txid(4),"vout":0}],"vout":[{"n":0,"value":if confirmed {0.0005} else {0.0001},"scriptPubKey":{"hex":script}}]})).unwrap());
        }
        assert_eq!(path, "/address/1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD/unspent/all");
        let results = serde_json::json!([{"tx_hash":txid(1),"tx_pos":0,"value":50000,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"},{"tx_hash":txid(2),"tx_pos":0,"value":20000,"height":10,"isSpentInMempoolTx":true,"status":"confirmed"},{"tx_hash":txid(5),"tx_pos":0,"value":1,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"},{"tx_hash":txid(3),"tx_pos":0,"value":10000,"isSpentInMempoolTx":false,"status":"unconfirmed"}]);
        Ok(serde_json::to_vec(&serde_json::json!({"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","error":"","result":results})).unwrap())
    }));
    let balance = client.get_balance_coin("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string()).await.unwrap();
    assert_eq!(balance.balance.available.to_string(), "50000");
    assert_eq!(balance.balance.pending_unconfirmed.to_string(), "10000");
    let utxos = client.get_utxos("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string()).await.unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].value.to_string(), "50000");
}
#[tokio::test]
async fn rejects_wrong_network_and_incomplete_paginated_results() {
    let client = provider(MockClient::new().with_get(|path| {
        Ok(match path {
            "/chain/info" => br#"{"chain":"test","blocks":1,"headers":1}"#.to_vec(),
            _ => br#"{"hash":"wrong"}"#.to_vec(),
        })
    }));
    assert!(client.get_chain_id().await.is_err());
    let client = provider(MockClient::new().with_get(|_| Ok(br#"{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","error":"","result":[],"nextPageToken":"next"}"#.to_vec())));
    assert!(client.get_balance_coin("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string()).await.is_err());
}
#[tokio::test]
async fn timeout_queries_same_transaction_without_rebroadcasting() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/sdk_vector.json")).unwrap();
    let raw = fixture["raw"].as_str().unwrap().to_string();
    let txid = fixture["txid"].as_str().unwrap().to_string();
    let expected = txid.clone();
    let posts = Arc::new(AtomicUsize::new(0));
    let calls = posts.clone();
    let client = provider(
        MockClient::new()
            .with_post(move |path, _| {
                assert_eq!(path, "/tx/raw");
                calls.fetch_add(1, Ordering::SeqCst);
                Err(ClientError::Timeout)
            })
            .with_get(move |path| {
                assert_eq!(path, format!("/tx/hash/{expected}"));
                Ok(serde_json::to_vec(&serde_json::json!({"txid":expected,"confirmations":0,"vin":[],"vout":[]})).unwrap())
            }),
    );
    assert_eq!(client.transaction_broadcast(raw, BroadcastOptions::default()).await.unwrap(), txid);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
}
#[test]
fn decimal_output_values_are_exact_even_in_scientific_notation() {
    for (value, expected) in [("0.00000001", 1), ("1e-8", 1), ("12.5000042", 1250000420)] {
        let output: super::models::TransactionOutput = serde_json::from_str(&format!(r#"{{"n":0,"value":{value},"scriptPubKey":{{"hex":""}}}}"#)).unwrap();
        assert_eq!(output.satoshis().unwrap(), expected);
    }
    let output: super::models::TransactionOutput = serde_json::from_str(r#"{"n":0,"value":0.000000001,"scriptPubKey":{"hex":""}}"#).unwrap();
    assert!(output.satoshis().is_err());
}

#[tokio::test]
async fn follows_real_next_page_token_contract() {
    let client = provider(MockClient::new().with_get(|path| {
        assert!(path.starts_with("/address/1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5/confirmed/history?limit=1000"));
        let first = !path.contains("&token=");
        let data = if first {
            include_str!("../../testdata/confirmed_history.json")
        } else {
            include_str!("../../testdata/confirmed_history_page2.json")
        };
        let mut data: serde_json::Value = serde_json::from_str(data).unwrap();
        if !first {
            data["nextPageToken"] = serde_json::Value::Null;
        }
        Ok(serde_json::to_vec(&data).unwrap())
    }));
    let history: Vec<super::models::History> = client
        .address_pages("1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", None, |token| BsvTarget::History {
            address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".to_string(),
            confirmation: Confirmation::Confirmed,
            token,
        })
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_ne!(history[0].tx_hash, history[1].tx_hash);
}

#[tokio::test]
async fn validates_source_value_and_owner_script_before_planning() {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let script = crate::BsvAddress::from_str(address).unwrap().script_pubkey();
    for (value, script, confirmations, expected) in [
        ("0.0005", hex::encode(script.as_bytes()), 1, true),
        ("0.0005", hex::encode(script.as_bytes()), 0, false),
        ("0.00049", hex::encode(script.as_bytes()), 1, false),
        ("0.0005", "a914000000000000000000000000000000000000000087".to_string(), 1, false),
    ] {
        let txid = format!("{:064}", 1);
        let client = provider(MockClient::new().with_get(move |path| {
            let response = if path.ends_with("/unspent/all") {
                serde_json::json!({"address":address,"error":"","result":[{"tx_hash":txid,"tx_pos":0,"value":50000,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"}]})
            } else {
                let value: serde_json::Number = serde_json::from_str(value).unwrap();
                serde_json::json!({"txid":txid,"confirmations":confirmations,"vin":[{"txid":format!("{:064}",2),"vout":0}],"vout":[{"n":0,"value":value,"scriptPubKey":{"hex":script}}]})
            };
            Ok(serde_json::to_vec(&response).unwrap())
        }));
        assert_eq!(client.get_utxos(address.to_string()).await.is_ok(), expected);
    }
}

#[test]
fn test_native_value_protects_ordinals_and_cosigned_tokens_above_one_satoshi() {
    let owner = crate::BsvAddress::from_str("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD").unwrap();
    let native = hex::encode(owner.script_pubkey().as_bytes());
    let cosigned = format!("{}ad21020a177d6a5e6f3a8689acd2e313bd1cf0dcf5a243d1cc67b7218602aee9e04b2fac", &native[..native.len() - 2]);
    let envelope = "0063036f726451106170706c69636174696f6e2f6a736f6e00027b7d68";
    for (satoshis, script, expected) in [
        (1000, native.clone(), Some(1000)),
        (1, native.clone(), None),
        (1000, cosigned.clone(), None),
        (1000, format!("{envelope}{cosigned}"), None),
        (1000, format!("{envelope}{native}"), None),
    ] {
        let output: super::models::TransactionOutput =
            serde_json::from_value(serde_json::json!({"n":0,"value":format!("0.{satoshis:08}").parse::<serde_json::Number>().unwrap(),"scriptPubKey":{"hex":script}})).unwrap();
        assert_eq!(output.native_value(&owner).unwrap(), expected);
    }
    for script in ["a914000000000000000000000000000000000000000087", "76a914000000000000000000000000000000000000000088ac"] {
        let output: super::models::TransactionOutput = serde_json::from_value(serde_json::json!({"n":0,"value":0.00001,"scriptPubKey":{"hex":script}})).unwrap();
        assert_eq!(output.native_value(&owner).unwrap_err().to_string(), "unsupported BSV source script or owner mismatch");
    }
}

#[tokio::test]
async fn test_multisig_funding_history_uses_script_target_without_crediting_members() {
    let owner = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let source_id = "33".repeat(32);
    let transaction_id = "11".repeat(32);
    let mut source: serde_json::Value = serde_json::from_str(include_str!("../../testdata/wallet_coinbase.json")).unwrap();
    source["txid"] = source_id.clone().into();
    source["vout"][0]["value"] = serde_json::from_str("0.001").unwrap();
    let mut funding: serde_json::Value = serde_json::from_str(include_str!("../../testdata/multisig_previous_info.json")).unwrap();
    funding["txid"] = transaction_id.clone().into();
    funding["vin"][0]["txid"] = source_id.clone().into();
    funding["vout"][0]["value"] = serde_json::from_str("0.00042").unwrap();
    funding["vout"][0]["scriptPubKey"]["addresses"] = serde_json::to_value(vec![owner]).unwrap();
    funding["vout"][1]["value"] = serde_json::from_str("0.000578").unwrap();
    funding["vout"][1]["scriptPubKey"]["hex"] = source["vout"][0]["scriptPubKey"]["hex"].clone();
    let output: super::models::TransactionOutput = serde_json::from_value(funding["vout"][0].clone()).unwrap();
    assert!(!output.belongs_to(&crate::BsvAddress::from_str(owner).unwrap()).unwrap());
    assert!(output.native_value(&crate::BsvAddress::from_str(owner).unwrap()).is_err());
    let expected_target = output.address().unwrap().unwrap();
    assert!(!crate::BsvAddress::is_valid(&expected_target));
    let expected = transaction_id.clone();
    let client = provider(MockClient::new().with_get(move |path| {
        let value = if path == format!("/tx/hash/{source_id}") {
            &source
        } else {
            assert_eq!(path, format!("/tx/hash/{expected}"));
            &funding
        };
        Ok(serde_json::to_vec(value).unwrap())
    }));
    let transaction = client
        .get_transaction_by_hash(TransactionIdRequest::new(Chain::BitcoinSv, transaction_id, None))
        .await
        .unwrap()
        .unwrap();
    let mapped = crate::finalize_wallet_transaction(transaction.clone(), &[owner.to_string()]).unwrap();
    assert_eq!(mapped.direction, primitives::TransactionDirection::Outgoing);
    assert_eq!(mapped.value, 42000u64.into());
    assert_eq!(mapped.fee, 200u64.into());
    assert_eq!(mapped.to, expected_target);
    assert!(crate::finalize_wallet_transaction(transaction, &["1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into()]).is_err());
}

#[tokio::test]
async fn test_missing_transaction_is_distinct_from_provider_failure() {
    let request = TransactionIdRequest::new(Chain::BitcoinSv, format!("{:064}", 1), None);
    let missing = provider(MockClient::new().with_get(|_| Err(ClientError::Http { status: 404, body: vec![] })));
    assert_eq!(missing.get_transaction_by_hash(request.clone()).await.unwrap(), None);
    let failed = provider(MockClient::new().with_get(|_| Err(ClientError::Http { status: 400, body: vec![] })));
    assert!(failed.get_transaction_by_hash(request).await.is_err());
}

#[tokio::test]
async fn test_native_balance_and_selection_exclude_same_wallet_tokens() {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let transaction_id = format!("{:064}", 1);
    let native = hex::encode(crate::BsvAddress::from_str(address).unwrap().script_pubkey().as_bytes());
    let cosigned = format!("{}ad21020a177d6a5e6f3a8689acd2e313bd1cf0dcf5a243d1cc67b7218602aee9e04b2fac", &native[..native.len() - 2]);
    let client = provider(MockClient::new().with_get(move |path| {
        let response = if path.ends_with("/unspent/all") {
            serde_json::json!({"address":address,"error":"","result":[{"tx_hash":transaction_id,"tx_pos":0,"value":50000,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"},{"tx_hash":transaction_id,"tx_pos":1,"value":1000,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"},{"tx_hash":transaction_id,"tx_pos":2,"value":1,"height":10,"isSpentInMempoolTx":false,"status":"confirmed"}]})
        } else {
            serde_json::json!({"txid":transaction_id,"confirmations":1,"vin":[{"txid":format!("{:064}",2),"vout":0}],"vout":[{"n":0,"value":0.0005,"scriptPubKey":{"hex":native}},{"n":1,"value":0.00001,"scriptPubKey":{"hex":cosigned}},{"n":2,"value":0.00000001,"scriptPubKey":{"hex":native}}]})
        };
        Ok(serde_json::to_vec(&response).unwrap())
    }));
    let balance = client.get_balance_coin(address.to_string()).await.unwrap();
    assert_eq!(balance.balance.available.to_string(), "50000");
    let utxos = client.get_utxos(address.to_string()).await.unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].vout, 0);
    assert_eq!(utxos[0].value.to_string(), "50000");
}

#[tokio::test]
async fn test_broadcast_checked_rejects_cancelled_scope_after_actual_scheduler_queue() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/sdk_vector.json")).unwrap();
    let raw = fixture["raw"].as_str().unwrap().to_string();
    let posts = Arc::new(AtomicUsize::new(0));
    let post_count = posts.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        post_count.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::Timeout)
    }));
    let queue = client.scheduler.hold_request_queue().await;
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let checks = AtomicUsize::new(0);
    let submitted = AtomicUsize::new(0);
    let metadata_reads = AtomicUsize::new(0);
    let (entered, wait) = futures::channel::oneshot::channel();
    let entered = std::sync::Mutex::new(Some(entered));
    let check = || {
        if checks.fetch_add(1, Ordering::SeqCst) == 0 {
            entered.lock().unwrap().take().unwrap().send(()).unwrap();
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err("scope cancelled".into());
        }
        Ok(())
    };
    let before = || {
        submitted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };
    let cancel = async {
        wait.await.unwrap();
        cancelled.store(true, Ordering::SeqCst);
        drop(queue);
    };
    let (result, ()) = futures::join!(
        client.broadcast_checked(
            raw,
            &check,
            || async {
                metadata_reads.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            &before
        ),
        cancel
    );
    assert!(result.is_err());
    assert_eq!(checks.load(Ordering::SeqCst), 2);
    assert_eq!(submitted.load(Ordering::SeqCst), 0);
    assert_eq!(metadata_reads.load(Ordering::SeqCst), 0);
    assert_eq!(posts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_broadcast_checked_marks_submission_only_at_post_and_preserves_observed_acceptance() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/sdk_vector.json")).unwrap();
    let raw = fixture["raw"].as_str().unwrap().to_string();
    let expected = fixture["txid"].as_str().unwrap().to_string();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let post_events = events.clone();
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let post_cancelled = cancelled.clone();
    let hash = expected.clone();
    let client = provider(MockClient::new().with_post(move |path, _| {
        assert_eq!(path, "/tx/raw");
        post_events.lock().unwrap().push("post");
        post_cancelled.store(true, Ordering::SeqCst);
        Ok(hash.as_bytes().to_vec())
    }));
    let check = || {
        events.lock().unwrap().push("check");
        if cancelled.load(Ordering::SeqCst) {
            return Err("scope cancelled".into());
        }
        Ok(())
    };
    let before = || {
        events.lock().unwrap().push("submission");
        Ok(())
    };
    let returned = client.broadcast_checked(raw, &check, || async { Ok(()) }, &before).await.unwrap();
    assert_eq!(returned, expected);
    assert_eq!(*events.lock().unwrap(), vec!["check", "check", "check", "submission", "post"]);
}

#[tokio::test]
async fn test_broadcast_checked_rechecks_cancellation_after_the_async_metadata_guard_without_marking_unknown() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/sdk_vector.json")).unwrap();
    let raw = fixture["raw"].as_str().unwrap().to_string();
    let posts = Arc::new(AtomicUsize::new(0));
    let post_count = posts.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        post_count.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::Timeout)
    }));
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let submitted = AtomicUsize::new(0);
    let check = || {
        if cancelled.load(Ordering::SeqCst) { Err("scope cancelled".into()) } else { Ok(()) }
    };
    let metadata = || async {
        cancelled.store(true, Ordering::SeqCst);
        Ok(())
    };
    let before = || {
        submitted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };
    assert!(client.broadcast_checked(raw, &check, metadata, &before).await.is_err());
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    assert_eq!(submitted.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_cold_broadcast_checked_rejects_cancelled_scope_after_actual_scheduler_queue_without_submission() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/bsv-cold-sdk-vectors.json")).unwrap();
    let raw = fixture["vectors"][0]["signed"]["rawTransaction"].as_str().unwrap().to_string();
    let posts = Arc::new(AtomicUsize::new(0));
    let count = posts.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        count.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::Timeout)
    }));
    let queue = client.scheduler.hold_request_queue().await;
    let cancelled = AtomicBool::new(false);
    let checks = AtomicUsize::new(0);
    let submitted = AtomicUsize::new(0);
    let (entered, wait) = oneshot::channel();
    let entered = Mutex::new(Some(entered));
    let check = || {
        if checks.fetch_add(1, Ordering::SeqCst) == 0 {
            entered.lock().unwrap().take().unwrap().send(()).unwrap();
        }
        if cancelled.load(Ordering::SeqCst) { Err("cold scope cancelled".into()) } else { Ok(()) }
    };
    let before_submission = || {
        submitted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };
    let cancel = async {
        wait.await.unwrap();
        cancelled.store(true, Ordering::SeqCst);
        drop(queue);
    };
    let (result, ()) = join!(client.broadcast_cold_transaction_checked(&raw, &check, || async { Ok(()) }, &before_submission), cancel);
    assert!(result.is_err());
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    assert_eq!(submitted.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_cold_broadcast_checked_acceptance_requires_exact_original_raw_not_another_valid_transaction() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/bsv-cold-sdk-vectors.json")).unwrap();
    let raw = fixture["vectors"][0]["signed"]["rawTransaction"].as_str().unwrap().to_string();
    let expected = fixture["vectors"][0]["signed"]["transactionId"].as_str().unwrap().to_string();
    let other = fixture["vectors"][1]["signed"]["rawTransaction"].as_str().unwrap().to_string();
    for (accepted, valid) in [(raw.clone(), true), (other, false)] {
        let reads = Arc::new(AtomicUsize::new(0));
        let get_count = reads.clone();
        let path = format!("/tx/{expected}/hex");
        let client = provider(MockClient::new().with_post(|_, _| Err(ClientError::Timeout)).with_get(move |requested| {
            assert_eq!(requested, path);
            get_count.fetch_add(1, Ordering::SeqCst);
            Ok(accepted.as_bytes().to_vec())
        }));
        let submissions = AtomicUsize::new(0);
        let before_submission = || {
            submissions.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let result = client.broadcast_cold_transaction_checked(&raw, &|| Ok(()), || async { Ok(()) }, &before_submission).await;
        assert_eq!(result.is_ok(), valid);
        assert_eq!(submissions.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        if valid {
            assert_eq!(result.unwrap(), expected);
        }
    }
}

#[tokio::test]
async fn test_cold_broadcast_checked_preserves_valid_acceptance_when_post_response_cancels_scope() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/bsv-cold-sdk-vectors.json")).unwrap();
    let raw = fixture["vectors"][0]["signed"]["rawTransaction"].as_str().unwrap().to_string();
    let expected = fixture["vectors"][0]["signed"]["transactionId"].as_str().unwrap().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let observed = cancelled.clone();
    let returned = expected.clone();
    let client = provider(MockClient::new().with_post(move |_, _| {
        observed.store(true, Ordering::SeqCst);
        Ok(returned.as_bytes().to_vec())
    }));
    let check = || if cancelled.load(Ordering::SeqCst) { Err("cold scope cancelled".into()) } else { Ok(()) };
    assert_eq!(
        client.broadcast_cold_transaction_checked(&raw, &check, || async { Ok(()) }, &|| Ok(())).await.unwrap(),
        expected
    );
}
