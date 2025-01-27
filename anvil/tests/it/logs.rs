//! log/event related tests

use crate::{abi::*, next_port};
use anvil::{spawn, NodeConfig};
use ethers::{
    middleware::SignerMiddleware,
    prelude::{BlockNumber, Filter, FilterKind, Middleware, Signer, H256},
    types::Log,
};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn get_past_events() {
    let (_api, handle) = spawn(NodeConfig::test().with_port(next_port())).await;
    let provider = handle.http_provider();

    let wallet = handle.dev_wallets().next().unwrap();
    let address = wallet.address();
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let contract = SimpleStorage::deploy(Arc::clone(&client), "initial value".to_string())
        .unwrap()
        .send()
        .await
        .unwrap();

    let func = contract.method::<_, H256>("setValue", "hi".to_owned()).unwrap();
    let tx = func.send().await.unwrap();
    let _receipt = tx.await.unwrap();

    // and we can fetch the events
    let logs: Vec<ValueChanged> =
        contract.event().from_block(0u64).topic1(address).query().await.unwrap();

    // 2 events, 1 in constructor, 1 in call
    assert_eq!(logs[0].new_value, "initial value");
    assert_eq!(logs[1].new_value, "hi");
    assert_eq!(logs.len(), 2);

    // and we can fetch the events at a block hash
    let hash = client.get_block(1).await.unwrap().unwrap().hash.unwrap();

    let logs: Vec<ValueChanged> =
        contract.event().at_block_hash(hash).topic1(address).query().await.unwrap();
    assert_eq!(logs[0].new_value, "initial value");
    assert_eq!(logs.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_all_events() {
    let (api, handle) = spawn(NodeConfig::test().with_port(next_port())).await;
    let provider = handle.http_provider();

    let wallet = handle.dev_wallets().next().unwrap();
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let contract = SimpleStorage::deploy(Arc::clone(&client), "initial value".to_string())
        .unwrap()
        .send()
        .await
        .unwrap();

    api.anvil_set_auto_mine(false).await.unwrap();

    let pre_logs = client.get_logs(&Filter::new().from_block(BlockNumber::Earliest)).await.unwrap();
    assert_eq!(pre_logs.len(), 1);

    let pre_logs =
        client.get_logs(&Filter::new().from_block(BlockNumber::Number(0u64.into()))).await.unwrap();
    assert_eq!(pre_logs.len(), 1);

    // spread logs across several blocks
    let num_tx = 10;
    for _ in 0..num_tx {
        let func = contract.method::<_, H256>("setValue", "hi".to_owned()).unwrap();
        let tx = func.send().await.unwrap();
        api.mine_one();
        let _receipt = tx.await.unwrap();
    }
    let logs = client.get_logs(&Filter::new().from_block(BlockNumber::Earliest)).await.unwrap();

    let num_logs = num_tx + pre_logs.len();
    assert_eq!(logs.len(), num_logs);
}

#[tokio::test(flavor = "multi_thread")]
async fn can_install_filter() {
    let (api, handle) = spawn(NodeConfig::test().with_port(next_port())).await;
    let provider = handle.http_provider();

    let wallet = handle.dev_wallets().next().unwrap();
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let contract = SimpleStorage::deploy(Arc::clone(&client), "initial value".to_string())
        .unwrap()
        .send()
        .await
        .unwrap();

    let filter = Filter::new().from_block(BlockNumber::Number(0u64.into()));

    let filter = client.new_filter(FilterKind::Logs(&filter)).await.unwrap();

    let logs = client.get_filter_changes::<_, Log>(filter).await.unwrap();
    assert_eq!(logs.len(), 1);

    let logs = client.get_filter_changes::<_, Log>(filter).await.unwrap();
    assert!(logs.is_empty());
    api.anvil_set_auto_mine(false).await.unwrap();
    // create some logs
    let num_logs = 10;
    for _ in 0..num_logs {
        let func = contract.method::<_, H256>("setValue", "hi".to_owned()).unwrap();
        let tx = func.send().await.unwrap();
        api.mine_one();
        let _receipt = tx.await.unwrap();
        let logs = client.get_filter_changes::<_, Log>(filter).await.unwrap();
        assert_eq!(logs.len(), 1);
    }
    let all_logs = api
        .get_filter_logs(&serde_json::to_string(&filter).unwrap().replace('\"', ""))
        .await
        .unwrap();

    assert_eq!(all_logs.len(), num_logs + 1);
}
