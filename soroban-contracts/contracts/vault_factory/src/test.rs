#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env, IntoVal, String, Vec, symbol_short,
};

use crate::{
    types::{BatchVaultParams, VaultType},
    VaultFactory, VaultFactoryClient,
};

mod single_rwa_vault {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/single_rwa_vault.wasm"
    );
}

const VAULT_WASM: &[u8] = include_bytes!("../../../target/wasm32-unknown-unknown/release/single_rwa_vault.wasm");

fn setup_factory(e: &Env) -> (VaultFactoryClient, Address, Address, Address, Address, BytesN<32>) {
    let admin = Address::generate(e);
    let asset = Address::generate(e);
    let zkme = Address::generate(e);
    let coop = Address::generate(e);
    
    // Upload the vault WASM
    let vault_wasm_hash = e.deployer().upload_contract_wasm(VAULT_WASM);

    let factory_id = e.register(
        VaultFactory,
        (
            admin.clone(),
            asset.clone(),
            zkme.clone(),
            coop.clone(),
            vault_wasm_hash.clone(),
        ),
    );
    
    (
        VaultFactoryClient::new(e, &factory_id),
        admin,
        asset,
        zkme,
        coop,
        vault_wasm_hash,
    )
}

#[test]
fn test_constructor_stores_defaults() {
    let e = Env::default();
    let (client, admin, asset, zkme, coop, _wasm_hash) = setup_factory(&e);

    assert_eq!(client.admin(), admin);
    assert_eq!(client.default_asset(), asset);
    assert_eq!(client.default_zkme_verifier(), zkme);
    assert_eq!(client.default_cooperator(), coop);
    // There isn't a direct getter for wasm_hash but we can verify operator status for admin
    assert!(client.is_operator(&admin));
}

#[test]
fn test_create_single_rwa_vault() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    let name = String::from_str(&e, "Test Vault");
    let symbol = String::from_str(&e, "TV");
    let rwa_name = String::from_str(&e, "Real Estate");
    let rwa_symbol = String::from_str(&e, "RE");
    let rwa_uri = String::from_str(&e, "https://example.com");
    let maturity = 1735689600u64; // arbitrary future date

    let vault_addr = client.create_single_rwa_vault(
        &admin,
        &asset,
        &name,
        &symbol,
        &rwa_name,
        &rwa_symbol,
        &rwa_uri,
        &maturity
    );

    // Verify registry
    assert!(client.is_registered_vault(&vault_addr));
    let all_vaults = client.get_all_vaults();
    assert!(all_vaults.contains(vault_addr.clone()));
    
    let info = client.get_vault_info(&vault_addr).unwrap();
    assert_eq!(info.name, name);
    assert_eq!(info.symbol, symbol);
    assert!(info.active);
    assert_eq!(info.vault_type, VaultType::SingleRwa);
}

#[test]
fn test_create_single_rwa_vault_full() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    let params = BatchVaultParams {
        asset: asset.clone(),
        name: String::from_str(&e, "Full Vault"),
        symbol: String::from_str(&e, "FV"),
        rwa_name: String::from_str(&e, "Private Credit"),
        rwa_symbol: String::from_str(&e, "PC"),
        rwa_document_uri: String::from_str(&e, "https://doc.com"),
        rwa_category: String::from_str(&e, "Finance"),
        expected_apy: 500u32, // 5%
        maturity_date: 1800000000u64,
        funding_deadline: 1750000000u64,
        funding_target: 1000000000i128,
        min_deposit: 100i128,
        max_deposit_per_user: 1000000i128,
        early_redemption_fee_bps: 100u32, // 1%
    };

    let vault_addr = client.create_single_rwa_vault_full(&admin, &params);

    assert!(client.is_registered_vault(&vault_addr));
    let info = client.get_vault_info(&vault_addr).unwrap();
    assert_eq!(info.name, params.name);
}

#[test]
fn test_batch_create_vaults() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    let mut batch = Vec::new(&e);
    for _i in 0..3 {
        batch.push_back(BatchVaultParams {
            asset: asset.clone(),
            name: String::from_str(&e, "Vault"),
            symbol: String::from_str(&e, "V"),
            rwa_name: String::from_str(&e, "RWA"),
            rwa_symbol: String::from_str(&e, "R"),
            rwa_document_uri: String::from_str(&e, "uri"),
            rwa_category: String::from_str(&e, "cat"),
            expected_apy: 0,
            maturity_date: 0,
            funding_deadline: 0,
            funding_target: 0,
            min_deposit: 0,
            max_deposit_per_user: 0,
            early_redemption_fee_bps: 0,
        });
    }

    let vaults = client.batch_create_vaults(&admin, &batch);
    assert_eq!(vaults.len(), 3);
    assert_eq!(client.get_vault_count(), 3);
}

#[test]
fn test_create_vault_emits_event() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    let name = String::from_str(&e, "Event Vault");
    client.create_single_rwa_vault(
        &admin,
        &asset,
        &name,
        &name, // symbol same as name
        &name,
        &name,
        &name,
        &0
    );

    let events = e.events().all();
    let last = events.last().expect("event must be emitted");
    
    // topics: (symbol_short!("v_create"), vault_addr, VaultType, name)
    let (_, topics, _) = last;
    let first_topic: soroban_sdk::Symbol = topics.get_unchecked(0).into_val(&e);
    assert_eq!(first_topic, symbol_short!("v_create"));
}

#[test]
fn test_get_active_vaults_filters_inactive() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    let v1 = client.create_single_rwa_vault(&admin, &asset, &String::from_str(&e, "V1"), &String::from_str(&e, "V1"), &String::from_str(&e, ""), &String::from_str(&e, ""), &String::from_str(&e, ""), &0);
    let v2 = client.create_single_rwa_vault(&admin, &asset, &String::from_str(&e, "V2"), &String::from_str(&e, "V2"), &String::from_str(&e, ""), &String::from_str(&e, ""), &String::from_str(&e, ""), &0);

    assert_eq!(client.get_active_vaults().len(), 2);

    client.set_vault_status(&admin, &v1, &false);
    
    let active = client.get_active_vaults();
    assert_eq!(active.len(), 1);
    assert!(active.contains(v2));
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_create_vault_non_operator_panics() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, asset, _, _, _) = setup_factory(&e);

    let rando = Address::generate(&e);
    client.create_single_rwa_vault(
        &rando,
        &asset,
        &String::from_str(&e, "Panic"),
        &String::from_str(&e, "P"),
        &String::from_str(&e, ""),
        &String::from_str(&e, ""),
        &String::from_str(&e, ""),
        &0
    );
}

#[test]
#[should_panic(expected = "Aggregator vault not supported")]
fn test_create_aggregator_vault_panics() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, asset, _, _, _) = setup_factory(&e);

    client.create_aggregator_vault(
        &admin,
        &asset,
        &String::from_str(&e, "No"),
        &String::from_str(&e, "N")
    );
}
