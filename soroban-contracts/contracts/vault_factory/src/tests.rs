extern crate std;

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env, String,
};

use crate::{
    errors::Error,
    storage::{delete_vault_info, get_all_vaults, get_single_rwa_vaults, get_vault_info,
              push_all_vaults, push_single_rwa_vaults, put_vault_info},
    types::{VaultInfo, VaultType},
    VaultFactory, VaultFactoryClient,
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Deploy and initialise a VaultFactory with a dummy WASM hash.
fn setup_factory(e: &Env) -> (VaultFactoryClient, Address) {
    let admin = Address::generate(e);
    let asset = Address::generate(e);
    let zkme = Address::generate(e);
    let coop = Address::generate(e);
    let wasm_hash = BytesN::<32>::from_array(e, &[0u8; 32]);

    let factory_id = e.register(
        VaultFactory,
        (
            admin.clone(),
            asset.clone(),
            zkme.clone(),
            coop.clone(),
            wasm_hash,
        ),
    );
    (VaultFactoryClient::new(e, &factory_id), admin)
}

/// Inject a vault record directly into factory storage, bypassing deployment.
/// Returns the generated vault address.
fn inject_vault(e: &Env, factory_id: &Address, active: bool) -> Address {
    let vault = Address::generate(e);
    let info = VaultInfo {
        vault: vault.clone(),
        vault_type: VaultType::SingleRwa,
        name: String::from_str(e, "Test Vault"),
        symbol: String::from_str(e, "TV"),
        active,
        created_at: e.ledger().timestamp(),
    };

    // Write inside the factory contract context so storage keys resolve
    // against the factory address.
    e.as_contract(factory_id, || {
        put_vault_info(e, &vault, info);
        push_all_vaults(e, vault.clone());
        push_single_rwa_vaults(e, vault.clone());
    });

    vault
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Admin successfully removes an inactive vault.
#[test]
fn test_remove_inactive_vault_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, admin) = setup_factory(&e);
    let factory_id = client.address.clone();

    let vault = inject_vault(&e, &factory_id, false /* inactive */);

    // Pre-conditions
    e.as_contract(&factory_id, || {
        assert!(get_vault_info(&e, &vault).is_some());
        assert!(get_all_vaults(&e).contains(vault.clone()));
        assert!(get_single_rwa_vaults(&e).contains(vault.clone()));
    });

    client.remove_vault(&admin, &vault);

    // Post-conditions: vault purged from all lists and VaultInfo deleted
    e.as_contract(&factory_id, || {
        assert!(get_vault_info(&e, &vault).is_none(), "VaultInfo must be deleted");
        assert!(
            !get_all_vaults(&e).contains(vault.clone()),
            "vault must not appear in AllVaults"
        );
        assert!(
            !get_single_rwa_vaults(&e).contains(vault.clone()),
            "vault must not appear in SingleRwaVaults"
        );
    });
}

/// get_all_vaults no longer returns the removed vault.
#[test]
fn test_get_all_vaults_excludes_removed_vault() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, admin) = setup_factory(&e);
    let factory_id = client.address.clone();

    // Two vaults; one will be removed
    let keep = inject_vault(&e, &factory_id, false);
    let remove = inject_vault(&e, &factory_id, false);

    client.remove_vault(&admin, &remove);

    let all = client.get_all_vaults();
    assert!(
        !all.contains(remove.clone()),
        "removed vault must not appear in get_all_vaults"
    );
    assert!(
        all.contains(keep.clone()),
        "remaining vault must still appear in get_all_vaults"
    );
}

/// Non-admin caller must be rejected with NotAuthorized.
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_remove_vault_non_admin_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, _admin) = setup_factory(&e);
    let factory_id = client.address.clone();
    let vault = inject_vault(&e, &factory_id, false);

    let random = Address::generate(&e);
    client.remove_vault(&random, &vault);
}

/// Attempting to remove an active vault must fail with VaultIsActive.
#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_remove_active_vault_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, admin) = setup_factory(&e);
    let factory_id = client.address.clone();
    let vault = inject_vault(&e, &factory_id, true /* active */);

    client.remove_vault(&admin, &vault);
}

/// Attempting to remove a vault that does not exist must fail with VaultNotFound.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_remove_unknown_vault_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, admin) = setup_factory(&e);
    let ghost = Address::generate(&e);

    client.remove_vault(&admin, &ghost);
}

/// VaultRemoved event is emitted on a successful removal.
#[test]
fn test_remove_vault_emits_event() {
    let e = Env::default();
    e.mock_all_auths();

    let (client, admin) = setup_factory(&e);
    let factory_id = client.address.clone();
    let vault = inject_vault(&e, &factory_id, false);

    client.remove_vault(&admin, &vault);

    // The last published event must carry the "v_remove" topic and the
    // vault address.
    let events = e.events().all();
    let last = events.last().expect("at least one event must be published");
    // topics: (symbol_short!("v_remove"), vault_addr)
    // data:   admin_addr
    let (contract, topics, _data) = last;
    assert_eq!(contract, factory_id);
    // Verify the first topic is the "v_remove" symbol
    let first_topic: soroban_sdk::Val = topics.get(0).unwrap();
    let expected: soroban_sdk::Val =
        soroban_sdk::symbol_short!("v_remove").into_val(&e);
    assert_eq!(first_topic, expected);
}
