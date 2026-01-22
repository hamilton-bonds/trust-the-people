//! Key generation commands for validator setup

use common::{Result, VotingError};
use crypto::keys::KeyPair;
use std::path::Path;
use tracing::info;

/// Generate a new validator keypair
pub fn generate_keys(output: &Path) -> Result<()> {
    info!("Generating new validator keypair...");
    
    // Generate new keypair
    let keypair = KeyPair::generate();
    
    // Save to file (no password for testnet keys)
    keypair.save_to_file(output, "")?;
    
    info!("Keypair saved to: {}", output.display());
    info!("Public key: {}", keypair.public_key());
    
    Ok(())
}

/// Show public key from a keypair file
pub fn show_public_key(keypair_path: &Path) -> Result<()> {
    info!("Reading keypair from: {}", keypair_path.display());
    
    // Load keypair (no password for testnet keys)
    let keypair = KeyPair::load_from_file(keypair_path, "")?;
    
    println!("Public Key: {}", keypair.public_key());
    
    Ok(())
}
