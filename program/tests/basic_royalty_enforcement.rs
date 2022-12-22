#![cfg(feature = "test-bpf")]

pub mod utils;

use mpl_token_auth_rules::{
    payload::{LeafInfo, Payload, PayloadKey, PayloadType},
    state::{Rule, RuleSet},
};
use rmp_serde::Serializer;
use serde::Serialize;
use solana_program::instruction::AccountMeta;
use solana_program_test::tokio;
use solana_sdk::{
    signature::Signer, signer::keypair::Keypair, system_instruction, transaction::Transaction,
};
use utils::{program_test, Operation};

#[tokio::test]
async fn basic_royalty_enforcement() {
    let mut context = program_test().start_with_context().await;

    // --------------------------------
    // Create RuleSet
    // --------------------------------
    // Find RuleSet PDA.
    let (rule_set_addr, _rule_set_bump) = mpl_token_auth_rules::pda::find_rule_set_address(
        context.payer.pubkey(),
        "basic_royalty_enforcement".to_string(),
    );

    // Rule for Transfers: Allow transfers to a Token Owned Escrow account.
    let owned_by_token_metadata = Rule::ProgramOwned {
        program: mpl_token_metadata::id(),
        field: PayloadKey::Target,
    };

    // Merkle tree root generated in a different test program.
    let marketplace_tree_root: [u8; 32] = [
        132, 141, 27, 31, 23, 154, 145, 128, 32, 62, 122, 224, 248, 128, 37, 139, 200, 46, 163,
        238, 76, 123, 155, 141, 73, 12, 111, 192, 122, 80, 126, 155,
    ];

    // Rule for Delegate and SaleTransfer: The provided leaf node must be a
    // member of the marketplace Merkle tree.
    let leaf_in_marketplace_tree = Rule::PubkeyTreeMatch {
        root: marketplace_tree_root,
        field: PayloadKey::Target,
    };

    // Create Basic Royalty Enforcement RuleSet.
    let mut basic_royalty_enforcement_rule_set = RuleSet::new(
        "basic_royalty_enforcement".to_string(),
        context.payer.pubkey(),
    );
    basic_royalty_enforcement_rule_set
        .add(Operation::Transfer.to_string(), owned_by_token_metadata)
        .unwrap();
    basic_royalty_enforcement_rule_set
        .add(
            Operation::Delegate.to_string(),
            leaf_in_marketplace_tree.clone(),
        )
        .unwrap();
    basic_royalty_enforcement_rule_set
        .add(
            Operation::SaleTransfer.to_string(),
            leaf_in_marketplace_tree,
        )
        .unwrap();

    println!(
        "{}",
        serde_json::to_string_pretty(&basic_royalty_enforcement_rule_set,).unwrap()
    );

    // Serialize the RuleSet using RMP serde.
    let mut serialized_data = Vec::new();
    basic_royalty_enforcement_rule_set
        .serialize(&mut Serializer::new(&mut serialized_data))
        .unwrap();

    // Create a `create` instruction.
    let create_ix = mpl_token_auth_rules::instruction::create(
        context.payer.pubkey(),
        rule_set_addr,
        serialized_data,
        vec![],
    );

    // Add it to a transaction.
    let create_tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    // Process the transaction.
    context
        .banks_client
        .process_transaction(create_tx)
        .await
        .expect("creation should succeed");

    // --------------------------------
    // Validate Transfer operation
    // --------------------------------
    // Create an account owned by token-metadata to simulate a Token-Owned Escrow account.
    let fake_token_metadata_owned_escrow = Keypair::new();
    let rent = context.banks_client.get_rent().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &context.payer.pubkey(),
            &fake_token_metadata_owned_escrow.pubkey(),
            rent.minimum_balance(0),
            0,
            &mpl_token_metadata::id(),
        )],
        Some(&context.payer.pubkey()),
        &[&context.payer, &fake_token_metadata_owned_escrow],
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await.unwrap();

    // Create a Keypair to simulate a token mint address.
    let mint = Keypair::new().pubkey();

    // Store the payload of data to validate against the rule definition.
    // In this case the Target will be used to look up the `AccountInfo`
    // and see who the owner is.
    let payload = Payload::from([(
        PayloadKey::Target,
        PayloadType::Pubkey(fake_token_metadata_owned_escrow.pubkey()),
    )]);

    // Create a `validate` instruction for a `Transfer` operation.
    let validate_ix = mpl_token_auth_rules::instruction::validate(
        rule_set_addr,
        mint,
        None,
        None,
        None,
        Operation::Transfer.to_string(),
        payload,
        false,
        vec![AccountMeta::new_readonly(
            fake_token_metadata_owned_escrow.pubkey(),
            false,
        )],
    );

    // Add it to a transaction.
    let validate_tx = Transaction::new_signed_with_payer(
        &[validate_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    // Process the transaction.
    context
        .banks_client
        .process_transaction(validate_tx)
        .await
        .expect("Transfer operation validation should succeed");

    // --------------------------------
    // Validate Delegate operation
    // --------------------------------
    // Merkle tree leaf node.
    let leaf: [u8; 32] = [
        2, 157, 245, 156, 21, 37, 147, 96, 42, 190, 206, 14, 24, 1, 106, 49, 167, 236, 38, 73, 98,
        53, 60, 9, 154, 31, 240, 126, 210, 197, 76, 7,
    ];

    // Merkle tree proof generated in a different test program.
    let proof: Vec<[u8; 32]> = vec![
        [
            246, 54, 96, 185, 234, 119, 124, 220, 54, 137, 25, 200, 18, 12, 114, 75, 211, 203, 154,
            229, 197, 53, 164, 84, 38, 56, 20, 74, 192, 119, 37, 175,
        ],
        [
            193, 84, 33, 232, 119, 107, 227, 166, 30, 233, 40, 10, 51, 229, 90, 59, 165, 212, 67,
            193, 159, 126, 26, 200, 13, 209, 162, 98, 52, 125, 240, 77,
        ],
        [
            238, 14, 13, 214, 124, 172, 89, 7, 66, 168, 226, 88, 92, 22, 18, 17, 94, 96, 37, 234,
            101, 96, 129, 26, 137, 222, 96, 86, 245, 11, 199, 140,
        ],
    ];

    let leaf_info = LeafInfo::new(leaf, proof);

    // Store the payload of data to validate against the rule definition.
    // In this case it is a leaf node and its associated Merkle proof.
    let payload = Payload::from([(PayloadKey::Target, PayloadType::MerkleProof(leaf_info))]);

    // Create a `validate` instruction for a `Delegate` operation.
    let validate_ix = mpl_token_auth_rules::instruction::validate(
        rule_set_addr,
        mint,
        None,
        None,
        None,
        Operation::Delegate.to_string(),
        payload.clone(),
        false,
        vec![],
    );

    // Add it to a transaction.
    let validate_tx = Transaction::new_signed_with_payer(
        &[validate_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    // Process the transaction.
    context
        .banks_client
        .process_transaction(validate_tx)
        .await
        .expect("Delegate operation validation should succeed");

    // --------------------------------
    // Validate SaleTransfer operation
    // --------------------------------
    // Create a `validate` instruction for a `SaleTransfer` operation.
    let validate_ix = mpl_token_auth_rules::instruction::validate(
        rule_set_addr,
        mint,
        None,
        None,
        None,
        Operation::SaleTransfer.to_string(),
        payload,
        false,
        vec![],
    );

    // Add it to a transaction.
    let validate_tx = Transaction::new_signed_with_payer(
        &[validate_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );

    // Process the transaction.
    context
        .banks_client
        .process_transaction(validate_tx)
        .await
        .expect("SaleTransfer operation validation should succeed");
}
