extern crate std;
use crate::test_helpers::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn test_multiple_deposit_times() {
    let ctx = setup_with_kyc_bypass();
    let env = &ctx.env;

    // User A and B
    let user_a = Address::generate(env);
    let user_b = Address::generate(env);

    // Set funding target to 1M to match the test deposit
    ctx.vault().set_funding_target(&ctx.admin, &1_000_000i128);

    // User A deposits early (1:1 price)
    mint_usdc(env, &ctx.asset_id, &user_a, 1_000_000);
    ctx.vault().deposit(&user_a, &1_000_000i128, &user_a);

    // Verify A shares
    assert_eq!(ctx.vault().balance(&user_a), 1_000_000);

    // Activate and distribute yield (100% yield)
    ctx.vault().activate_vault(&ctx.operator);
    mint_usdc(env, &ctx.asset_id, &ctx.operator, 1_000_000);
    ctx.vault().distribute_yield(&ctx.operator, &1_000_000i128);

    // Current state: 2M assets, 1M shares -> Price = 2.0

    // User B deposits later
    mint_usdc(env, &ctx.asset_id, &user_b, 1_000_000);
    ctx.vault().deposit(&user_b, &1_000_000i128, &user_b);

    // B should get 500k shares (1M assets / 2.0 price)
    assert_eq!(ctx.vault().balance(&user_b), 500_000);

    // Distribute more yield (1.5M assets)
    mint_usdc(env, &ctx.asset_id, &ctx.operator, 1_500_000);
    ctx.vault().distribute_yield(&ctx.operator, &1_500_000i128);

    // Total shares = 1.5M. Yield = 1.5M.
    // Yield per share = 1.0.

    // Pending yield for A (1M shares) = 1M (epoch 1) + 1M (epoch 2) = 2M
    // Pending yield for B (0.5M shares) = 0.5M (epoch 2)

    assert_eq!(ctx.vault().pending_yield(&user_a), 2_000_000);
    assert_eq!(ctx.vault().pending_yield(&user_b), 500_000);

    // Verify shares remain correct
    assert_eq!(ctx.vault().balance(&user_a), 1_000_000);
    assert_eq!(ctx.vault().balance(&user_b), 500_000);
}
