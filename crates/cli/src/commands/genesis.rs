//! Genesis block generation commands

use blockchain_core::genesis::{GenesisBlock, GenesisBuilder};
use common::{Result, VotingError};
use crypto::keys::KeyPair;
use std::path::Path;
use tracing::info;

/// Generate a genesis block from validator keypairs
pub fn generate_genesis(
    validator_keys: &str,
    chain_id: &str,
    output: &Path,
) -> Result<()> {
    info!("Generating genesis block...");
    info!("Chain ID: {}", chain_id);
    
    // Parse comma-separated validator key paths
    let key_paths: Vec<&str> = validator_keys.split(',').collect();
    info!("Loading {} validator keys", key_paths.len());
    
    if key_paths.is_empty() {
        return Err(VotingError::ConfigError(
            "At least one validator key required".to_string()
        ));
    }
    
    // Load public keys from each keypair file
    let mut public_keys = Vec::new();
    for (i, path) in key_paths.iter().enumerate() {
        let path = path.trim();
        info!("  Loading validator {}: {}", i + 1, path);
        
        // Load keypair (no password for testnet keys)
        let keypair = KeyPair::load_from_file(Path::new(path), "")
            .map_err(|e| VotingError::ConfigError(
                format!("Failed to load key from {}: {}", path, e)
            ))?;
        
        // Convert crypto::PublicKey to common::PublicKey
        let crypto_pubkey = keypair.public_key();
        let common_pubkey = common::PublicKey::new(*crypto_pubkey.as_bytes());
        
        public_keys.push(common_pubkey);
        info!("    Public key: {}", crypto_pubkey);
    }
    
    // Build genesis block
    let genesis = GenesisBuilder::new()
        .add_validators(public_keys)
        .chain_id(chain_id.to_string())
        .network_name(format!("{} Network", chain_id))
        .timestamp(common::utils::current_timestamp())
        .build();
    
    // Save to file
    genesis.to_file(output.to_str().unwrap())?;
    
    info!("Genesis block created successfully!");
    info!("Output: {}", output.display());
    info!("Genesis hash: {}", genesis.calculate_hash()?);
    
    Ok(())
}
