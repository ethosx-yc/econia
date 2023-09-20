use std::{time::Duration, str::FromStr};

use anyhow::Result;
use aptos_sdk::{
    bcs,
    move_types::{
        ident_str,
        language_storage::{ModuleId, StructTag, TypeTag},
    },
    rest_client::{aptos_api_types::MoveModuleId, FaucetClient},
    types::{
        account_address::AccountAddress, transaction::EntryFunction, LocalAccount
    },
};
use clap::Parser;
use econia_sdk::EconiaClient;
use sqlx::PgPool;

pub const TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Parser, Debug)]
pub struct Args {
    /// The URL of the Aptos node
    pub node_url: String,

    /// The URL of the faucet
    pub faucet_url: String,

    /// The address of the Econia contract
    pub econia_address: String,

    /// The address of the Aptos faucet
    pub faucet_address: String,

    /// The database URL
    pub db_url: String,
}

pub struct State {
    pub e_apt: TypeTag,
    pub e_usdc: TypeTag,
    pub faucet_address: AccountAddress,
    pub faucet_client: FaucetClient,
    pub econia_address: AccountAddress,
    pub econia_client: EconiaClient,
    pub db_pool: PgPool,
    pub node_url: String,
}

/// Creates the initial variables needed
pub async fn init(args: &Args) -> State {
    // Create eAPT and eUSDC `TypeTag`s
    let faucet_address = AccountAddress::from_hex_literal(&args.faucet_address).unwrap();
    let e_apt = TypeTag::Struct(Box::new(
        StructTag::from_str(&format!("0x{faucet_address}::example_apt::ExampleAPT")).unwrap(),
    ));
    let e_usdc = TypeTag::Struct(Box::new(
        StructTag::from_str(&format!("0x{faucet_address}::example_usdc::ExampleUSDC")).unwrap(),
    ));

    // Create a `FaucetClient`
    let faucet_client = FaucetClient::new(
        reqwest::Url::parse(&args.faucet_url).unwrap(),
        reqwest::Url::parse(&args.node_url).unwrap(),
    );

    // Transform the Econia address from `String` to `AccountAddress`
    let econia_address =
        AccountAddress::from_hex_literal(&args.econia_address).expect("Could not parse address.");

    let (_, econia_client) = account(&faucet_client, &args.node_url, econia_address.clone()).await;

    let db_pool = PgPool::connect(&args.db_url)
        .await
        .expect("Could not connect to the database.");

    State {
        e_apt,
        e_usdc,
        faucet_address,
        faucet_client,
        econia_address,
        econia_client,
        db_pool,
        node_url: args.node_url.clone()
    }
}

/// Creates an account (locally and on the chain) and funds it with APT
pub async fn account(
    faucet_client: &FaucetClient,
    node_url: &str,
    econia_address: AccountAddress,
) -> (AccountAddress, EconiaClient) {
    let account = LocalAccount::generate(&mut rand::thread_rng());
    let account_address = account.address();
    faucet_client.create_account(account_address).await.unwrap();
    faucet_client
        .fund(account_address, 100_000_000_000)
        .await
        .unwrap();

    let econia_client = EconiaClient::connect(
        reqwest::Url::parse(&node_url).unwrap(),
        econia_address.clone(),
        account,
        None,
    )
    .await
    .unwrap();

    (account_address, econia_client)
}

/// Funds an amount with the coin specified
pub async fn fund(
    coin: &TypeTag,
    amount: u64,
    econia_client: &mut EconiaClient,
    faucet_address: AccountAddress,
) -> Result<()> {
    let module_id = ModuleId::from(
        MoveModuleId::from_str(&format!("{}::faucet", faucet_address))?
    );
    let entry = EntryFunction::new(
        module_id.clone(),
        ident_str!("mint").to_owned(),
        vec![coin.clone().into()],
        vec![bcs::to_bytes(&amount)?],
    );
    econia_client.submit_tx(entry).await?;
    Ok(())
}
