use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bsv_home_client::{ClientError, testkit::MockClient};
use primitives::TransactionDirection;

use crate::rpc::BsvRequestScheduler;

use super::*;

fn provider(client: MockClient) -> BsvClient<MockClient> {
    BsvClient::with_scheduler(client, Arc::new(BsvRequestScheduler::default()))
}

fn owners() -> Vec<String> {
    let vector: serde_json::Value = serde_json::from_str(include_str!("../../testdata/multiple_keys_vector.json")).unwrap();
    vector["input_addresses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|address| address.as_str().unwrap().to_string())
        .collect()
}

fn history(address: &str, transaction_id: &str) -> Vec<u8> {
    let mut response: serde_json::Value = serde_json::from_str(include_str!("../../testdata/wallet_history.json")).unwrap();
    response[0]["address"] = address.into();
    response[0]["confirmed"]["result"][0]["tx_hash"] = transaction_id.into();
    serde_json::to_vec(&response).unwrap()
}

#[tokio::test]
async fn test_address_activity_uses_proven_history_even_for_one_satoshi() {
    let address = owners().remove(0);
    for confirmed in [true, false] {
        let history_address = address.clone();
        let client = provider(MockClient::new().with_post(move |_, _| Ok(history(&history_address, &"1".repeat(64)))).with_get(move |_| {
            let mut transaction: serde_json::Value = serde_json::from_str(include_str!("../../testdata/activity_one_sat.json")).unwrap();
            if !confirmed {
                transaction["confirmations"] = 0.into();
                transaction["blockheight"] = serde_json::Value::Null;
            }
            Ok(serde_json::to_vec(&transaction).unwrap())
        }));
        assert_eq!(client.address_activity(&address).await.unwrap(), (true, confirmed));
    }
    let unrelated = owners().remove(1);
    let history_address = unrelated.clone();
    let client = provider(
        MockClient::new()
            .with_post(move |_, _| Ok(history(&history_address, &"1".repeat(64))))
            .with_get(|_| Ok(include_bytes!("../../testdata/activity_one_sat.json").to_vec())),
    );
    assert!(client.address_activity(&unrelated).await.is_err());
}

#[tokio::test]
async fn test_bulk_address_activities_preserve_order_and_verify_each_owner() {
    let mut addresses = owners();
    addresses.reverse();
    let owner = addresses[1].clone();
    let posts = Arc::new(AtomicUsize::new(0));
    let calls = posts.clone();
    let client = provider(
        MockClient::new()
            .with_post(move |_, body| {
                calls.fetch_add(1, Ordering::SeqCst);
                let body: serde_json::Value = serde_json::from_slice(body).unwrap();
                let responses = body["addresses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .map(|address| {
                        let mut response: serde_json::Value = serde_json::from_slice(&history(address.as_str().unwrap(), &"1".repeat(64))).unwrap();
                        if address.as_str().unwrap() != owner {
                            response[0]["confirmed"]["result"] = Vec::<serde_json::Value>::new().into();
                        }
                        response[0].clone()
                    })
                    .collect::<Vec<_>>();
                Ok(serde_json::to_vec(&responses).unwrap())
            })
            .with_get(|_| Ok(include_bytes!("../../testdata/activity_one_sat.json").to_vec())),
    );
    assert_eq!(client.address_activities(&addresses).await.unwrap(), [(false, false), (true, true)]);
    assert_eq!(posts.load(Ordering::SeqCst), 1);

    let client = provider(
        MockClient::new()
            .with_post(|_, body| {
                let body: serde_json::Value = serde_json::from_slice(body).unwrap();
                let responses = body["addresses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|address| {
                        let response: serde_json::Value = serde_json::from_slice(&history(address.as_str().unwrap(), &"1".repeat(64))).unwrap();
                        response[0].clone()
                    })
                    .collect::<Vec<_>>();
                Ok(serde_json::to_vec(&responses).unwrap())
            })
            .with_get(|_| Ok(include_bytes!("../../testdata/activity_one_sat.json").to_vec())),
    );
    assert!(client.address_activities(&owners()).await.is_err());
}

#[tokio::test]
async fn test_address_activity_counts_protected_outputs_and_spent_input_history() {
    let address = owners().remove(0);
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../testdata/activity_one_sat.json")).unwrap();
    let native = fixture["vout"][0]["scriptPubKey"]["hex"].as_str().unwrap();
    let envelope = "0063036f726451106170706c69636174696f6e2f6a736f6e00027b7d68";
    let cosigned = format!("{}ad21020a177d6a5e6f3a8689acd2e313bd1cf0dcf5a243d1cc67b7218602aee9e04b2fac", &native[..native.len() - 2]);
    for script in [format!("{envelope}{native}"), cosigned] {
        let history_address = address.clone();
        let mut transaction = fixture.clone();
        transaction["vout"][0]["scriptPubKey"]["hex"] = script.into();
        let client = provider(
            MockClient::new()
                .with_post(move |_, _| Ok(history(&history_address, &"1".repeat(64))))
                .with_get(move |_| Ok(serde_json::to_vec(&transaction).unwrap())),
        );
        assert_eq!(client.address_activity(&address).await.unwrap(), (true, true));
    }
    let history_address = address.clone();
    let client = provider(MockClient::new().with_post(move |_, _| Ok(history(&history_address, &"2".repeat(64)))).with_get(|path| {
        if path.ends_with(&"2".repeat(64)) {
            Ok(include_bytes!("../../testdata/wallet_spent_activity.json").to_vec())
        } else {
            Ok(include_bytes!("../../testdata/activity_one_sat.json").to_vec())
        }
    }));
    assert_eq!(client.address_activity(&address).await.unwrap(), (true, true));
}

#[tokio::test]
async fn test_wallet_transactions_deduplicate_before_fetch_and_count_owned_outputs_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let requests = count.clone();
    let posts = Arc::new(AtomicUsize::new(0));
    let history_requests = posts.clone();
    let client = provider(
        MockClient::new()
            .with_post(move |path, body| {
                assert_eq!(path, "/addresses/history/all");
                history_requests.fetch_add(1, Ordering::SeqCst);
                let body: serde_json::Value = serde_json::from_slice(body).unwrap();
                let responses = body["addresses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|address| {
                        let response: serde_json::Value = serde_json::from_slice(&history(address.as_str().unwrap(), &"3".repeat(64))).unwrap();
                        response[0].clone()
                    })
                    .collect::<Vec<_>>();
                Ok(serde_json::to_vec(&responses).unwrap())
            })
            .with_get(move |_| {
                requests.fetch_add(1, Ordering::SeqCst);
                Ok(include_bytes!("../../testdata/wallet_coinbase.json").to_vec())
            }),
    );
    let transactions = client.wallet_transactions(&owners(), 1000).await.unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].value, 80000u64.into());
    assert_eq!(transactions[0].direction, TransactionDirection::Incoming);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_wallet_transactions_batch_preserves_address_pagination() {
    let posts = Arc::new(AtomicUsize::new(0));
    let pages = Arc::new(AtomicUsize::new(0));
    let history_requests = posts.clone();
    let page_requests = pages.clone();
    let addresses = owners();
    let page_addresses = addresses.clone();
    let client = provider(
        MockClient::new()
            .with_post(move |_, body| {
                history_requests.fetch_add(1, Ordering::SeqCst);
                let body: serde_json::Value = serde_json::from_slice(body).unwrap();
                let responses = body["addresses"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .map(|address| {
                        let mut response: serde_json::Value = serde_json::from_slice(&history(address.as_str().unwrap(), &"3".repeat(64))).unwrap();
                        for section in ["confirmed", "unconfirmed"] {
                            response[0][section]["result"] = Vec::<serde_json::Value>::new().into();
                            response[0][section]["nextPageToken"] = section.into();
                        }
                        response[0].clone()
                    })
                    .collect::<Vec<_>>();
                Ok(serde_json::to_vec(&responses).unwrap())
            })
            .with_get(move |path| {
                if path == format!("/tx/hash/{}", "3".repeat(64)) {
                    return Ok(include_bytes!("../../testdata/wallet_coinbase.json").to_vec());
                }
                page_requests.fetch_add(1, Ordering::SeqCst);
                let address = page_addresses.iter().find(|address| path.starts_with(&format!("/address/{address}/"))).unwrap();
                assert!(
                    path == format!("/address/{address}/confirmed/history?limit=1000&token=confirmed")
                        || path == format!("/address/{address}/unconfirmed/history?limit=1000&token=unconfirmed")
                );
                let response: serde_json::Value = serde_json::from_slice(&history(address, &"3".repeat(64))).unwrap();
                let mut page = response[0]["confirmed"].clone();
                page["address"] = address.clone().into();
                Ok(serde_json::to_vec(&page).unwrap())
            }),
    );
    let transactions = client.wallet_transactions(&addresses, 1000).await.unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert_eq!(pages.load(Ordering::SeqCst), addresses.len() * 2);
}

#[tokio::test]
async fn test_wallet_unspent_does_not_return_a_partial_snapshot() {
    let addresses = owners();
    let first = addresses[0].clone();
    let client = provider(MockClient::new().with_get(move |path| {
        if path == format!("/address/{first}/unspent/all") {
            let mut response: serde_json::Value = serde_json::from_str(include_str!("../../testdata/empty_unspent.json")).unwrap();
            response["address"] = first.clone().into();
            Ok(serde_json::to_vec(&response).unwrap())
        } else {
            Err(ClientError::Http { status: 403, body: Vec::new() })
        }
    }));
    assert!(client.wallet_native_unspent(&addresses).await.is_err());
    assert!(client.wallet_native_unspent(&[]).await.is_err());
    assert!(client.wallet_native_unspent(&[addresses[0].clone(), addresses[0].clone()]).await.is_err());
}
