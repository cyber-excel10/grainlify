use grainlify_core::asset::{
    balance, normalize_asset_id, transfer_exact, validate_asset_id, AssetIdError,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::Address as _,
    token, Address, Env,
};

#[contracttype]
#[derive(Clone)]
enum FeeOnTransferDataKey {
    Balance(Address),
    FeeBps,
}

#[contract]
struct FeeOnTransferToken;

#[contractimpl]
impl FeeOnTransferToken {
    pub fn init(env: Env, fee_bps: i128) {
        env.storage()
            .instance()
            .set(&FeeOnTransferDataKey::FeeBps, &fee_bps);
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let next = read_mock_balance(&env, &to) + amount;
        write_mock_balance(&env, &to, next);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        read_mock_balance(&env, &id)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let fee_bps: i128 = env
            .storage()
            .instance()
            .get(&FeeOnTransferDataKey::FeeBps)
            .unwrap_or(0);
        let fee = amount.saturating_mul(fee_bps) / 10_000;
        let received = amount.saturating_sub(fee);

        write_mock_balance(&env, &from, read_mock_balance(&env, &from) - amount);
        write_mock_balance(&env, &to, read_mock_balance(&env, &to) + received);
    }
}

fn read_mock_balance(env: &Env, holder: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&FeeOnTransferDataKey::Balance(holder.clone()))
        .unwrap_or(0)
}

fn write_mock_balance(env: &Env, holder: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&FeeOnTransferDataKey::Balance(holder.clone()), &amount);
}

#[test]
fn accepts_contract_address_asset_id() {
    let env = Env::default();
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);

    assert_eq!(validate_asset_id(&env, &sac.address()), Ok(()));
}

#[test]
fn normalize_asset_id_returns_original_contract_address() {
    let env = Env::default();
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);

    assert_eq!(normalize_asset_id(&env, &sac.address()), Ok(sac.address()));
}

#[test]
fn rejects_account_address_asset_id() {
    let env = Env::default();
    let issuer_admin = Address::generate(&env);
    let stellar_asset = env.register_stellar_asset_contract_v2(issuer_admin);
    let account_address = stellar_asset.issuer().address();

    assert_eq!(
        validate_asset_id(&env, &account_address),
        Err(AssetIdError::MustBeContractAddress)
    );
}

#[test]
fn reads_balance_from_sac_token_client() {
    let env = Env::default();
    env.mock_all_auths();
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = sac.address();
    let admin_client = token::StellarAssetClient::new(&env, &token_address);
    let holder = Address::generate(&env);

    admin_client.mint(&holder, &700);

    assert_eq!(balance(&env, &token_address, &holder), Ok(700));
}

#[test]
fn transfer_exact_accepts_standard_sac_transfers() {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = sac.address();
    let admin_client = token::StellarAssetClient::new(&env, &token_address);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    admin_client.mint(&sender, &1_000);

    assert_eq!(transfer_exact(&env, &token_address, &sender, &recipient, 250), Ok(()));
    assert_eq!(balance(&env, &token_address, &sender), Ok(750));
    assert_eq!(balance(&env, &token_address, &recipient), Ok(250));
}

#[test]
fn transfer_exact_rejects_fee_on_transfer_tokens() {
    let env = Env::default();
    env.mock_all_auths();

    let token_address = env.register_contract(None, FeeOnTransferToken);
    let token_client = FeeOnTransferTokenClient::new(&env, &token_address);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    token_client.init(&500);
    token_client.mint(&sender, &1_000);

    assert_eq!(
        transfer_exact(&env, &token_address, &sender, &recipient, 1_000),
        Err(AssetIdError::TransferAmountMismatch)
    );
}

#[test]
fn transfer_exact_is_noop_for_zero_amount_or_same_party() {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = sac.address();
    let admin_client = token::StellarAssetClient::new(&env, &token_address);
    let holder = Address::generate(&env);

    admin_client.mint(&holder, &321);

    assert_eq!(transfer_exact(&env, &token_address, &holder, &holder, 321), Ok(()));
    assert_eq!(transfer_exact(&env, &token_address, &holder, &holder, 0), Ok(()));
    assert_eq!(balance(&env, &token_address, &holder), Ok(321));
}
