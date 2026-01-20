use common::{BlockHash, Hash};
use serde::{Deserialize, Serialize};

/// Merkle tree for efficient verification of transaction sets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleTree {
    /// Root hash of the tree
    root: BlockHash,
    
    /// All leaf hashes (transaction hashes)
    leaves: Vec<BlockHash>,
    
    /// Internal nodes of the tree (organized by levels)
    levels: Vec<Vec<BlockHash>>,
}

impl MerkleTree {
    /// Create a new Merkle tree from a list of hashes
    pub fn new(hashes: Vec<BlockHash>) -> Self {
        if hashes.is_empty() {
            return Self {
                root: BlockHash::zero(),
                leaves: vec![],
                levels: vec![],
            };
        }
        
        let leaves = hashes.clone();
        let levels = Self::build_tree(hashes);
        let root = levels.last().unwrap()[0];
        
        Self {
            root,
            leaves,
            levels,
        }
    }
    
    /// Build the Merkle tree levels bottom-up
    fn build_tree(mut current_level: Vec<BlockHash>) -> Vec<Vec<BlockHash>> {
        let mut levels = vec![current_level.clone()];
        
        while current_level.len() > 1 {
            current_level = Self::build_next_level(&current_level);
            levels.push(current_level.clone());
        }
        
        levels
    }
    
    /// Build the next level of the tree by hashing pairs
    fn build_next_level(level: &[BlockHash]) -> Vec<BlockHash> {
        let mut next_level = Vec::new();
        let mut i = 0;
        
        while i < level.len() {
            if i + 1 < level.len() {
                // Hash pair of nodes
                let combined = Self::hash_pair(&level[i], &level[i + 1]);
                next_level.push(combined);
                i += 2;
            } else {
                // Odd node - duplicate it
                let combined = Self::hash_pair(&level[i], &level[i]);
                next_level.push(combined);
                i += 1;
            }
        }
        
        next_level
    }
    
    /// Hash two nodes together
    fn hash_pair(left: &BlockHash, right: &BlockHash) -> BlockHash {
        let combined = common::utils::hash_multiple(&[left.as_bytes(), right.as_bytes()]);
        BlockHash::new(combined)
    }
    
    /// Get the root hash
    pub fn root(&self) -> BlockHash {
        self.root
    }
    
    /// Get the leaf hashes
    pub fn leaves(&self) -> &[BlockHash] {
        &self.leaves
    }
    
    /// Get the number of leaves
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
    
    /// Generate a Merkle proof for a specific leaf
    pub fn generate_proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaves.len() {
            return None;
        }
        
        let mut proof_hashes = Vec::new();
        let mut current_index = leaf_index;
        
        // Traverse up the tree collecting sibling hashes
        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            // Get sibling hash (or duplicate if it doesn't exist)
            let sibling_hash = if sibling_index < level.len() {
                level[sibling_index]
            } else {
                level[current_index]
            };
            
            proof_hashes.push(ProofNode {
                hash: sibling_hash,
                is_left: current_index % 2 == 1,
            });
            
            current_index /= 2;
        }
        
        Some(MerkleProof {
            leaf_hash: self.leaves[leaf_index],
            leaf_index,
            proof: proof_hashes,
            root: self.root,
        })
    }
    
    /// Verify that a leaf exists in the tree using a proof
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current_hash = proof.leaf_hash;
        
        for node in &proof.proof {
            current_hash = if node.is_left {
                Self::hash_pair(&node.hash, &current_hash)
            } else {
                Self::hash_pair(&current_hash, &node.hash)
            };
        }
        
        current_hash == proof.root
    }
    
    /// Get the depth of the tree
    pub fn depth(&self) -> usize {
        self.levels.len()
    }
    
    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

/// A Merkle proof for a single leaf
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleProof {
    /// The leaf hash being proven
    pub leaf_hash: BlockHash,
    
    /// Index of the leaf in the tree
    pub leaf_index: usize,
    
    /// Proof path (sibling hashes from leaf to root)
    pub proof: Vec<ProofNode>,
    
    /// Root hash of the tree
    pub root: BlockHash,
}

/// A node in the Merkle proof path
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofNode {
    /// Hash of the sibling node
    pub hash: BlockHash,
    
    /// True if this node is on the left side
    pub is_left: bool,
}

impl MerkleProof {
    /// Verify this proof
    pub fn verify(&self) -> bool {
        MerkleTree::verify_proof(self)
    }
    
    /// Get the size of the proof (number of hashes)
    pub fn size(&self) -> usize {
        self.proof.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_hashes(count: usize) -> Vec<BlockHash> {
        (0..count)
            .map(|i| {
                let mut hash = [0u8; 32];
                hash[0] = i as u8;
                BlockHash::new(hash)
            })
            .collect()
    }

    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new(vec![]);
        assert!(tree.is_empty());
        assert_eq!(tree.root(), BlockHash::zero());
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn test_single_leaf() {
        let hashes = create_test_hashes(1);
        let tree = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.root(), hashes[0]);
    }

    #[test]
    fn test_two_leaves() {
        let hashes = create_test_hashes(2);
        let tree = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree.leaf_count(), 2);
        assert_eq!(tree.depth(), 2);
        
        // Root should be hash of the two leaves
        let expected_root = MerkleTree::hash_pair(&hashes[0], &hashes[1]);
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn test_four_leaves() {
        let hashes = create_test_hashes(4);
        let tree = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree.leaf_count(), 4);
        assert_eq!(tree.depth(), 3);
    }

    #[test]
    fn test_odd_leaves() {
        let hashes = create_test_hashes(3);
        let tree = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree.leaf_count(), 3);
        assert_eq!(tree.depth(), 3);
    }

    #[test]
    fn test_generate_proof() {
        let hashes = create_test_hashes(4);
        let tree = MerkleTree::new(hashes.clone());
        
        let proof = tree.generate_proof(0);
        assert!(proof.is_some());
        
        let proof = proof.unwrap();
        assert_eq!(proof.leaf_hash, hashes[0]);
        assert_eq!(proof.leaf_index, 0);
        assert_eq!(proof.root, tree.root());
    }

    #[test]
    fn test_verify_proof() {
        let hashes = create_test_hashes(4);
        let tree = MerkleTree::new(hashes.clone());
        
        // Generate and verify proof for each leaf
        for i in 0..4 {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify());
            assert!(MerkleTree::verify_proof(&proof));
        }
    }

    #[test]
    fn test_invalid_proof() {
        let hashes = create_test_hashes(4);
        let tree = MerkleTree::new(hashes.clone());
        
        let mut proof = tree.generate_proof(0).unwrap();
        
        // Tamper with the proof
        proof.leaf_hash = BlockHash::new([99u8; 32]);
        
        assert!(!proof.verify());
    }

    #[test]
    fn test_proof_size() {
        let hashes = create_test_hashes(8);
        let tree = MerkleTree::new(hashes.clone());
        
        let proof = tree.generate_proof(0).unwrap();
        
        // For 8 leaves, proof size should be 3 (log2(8))
        assert_eq!(proof.size(), 3);
    }

    #[test]
    fn test_deterministic_root() {
        let hashes = create_test_hashes(4);
        
        let tree1 = MerkleTree::new(hashes.clone());
        let tree2 = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_different_roots() {
        let hashes1 = create_test_hashes(4);
        let hashes2 = create_test_hashes(5);
        
        let tree1 = MerkleTree::new(hashes1);
        let tree2 = MerkleTree::new(hashes2);
        
        assert_ne!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_large_tree() {
        let hashes = create_test_hashes(1000);
        let tree = MerkleTree::new(hashes.clone());
        
        assert_eq!(tree.leaf_count(), 1000);
        
        // Verify random proofs
        for i in [0, 100, 500, 999] {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify());
        }
    }

    #[test]
    fn test_proof_index_out_of_bounds() {
        let hashes = create_test_hashes(4);
        let tree = MerkleTree::new(hashes);
        
        let proof = tree.generate_proof(10);
        assert!(proof.is_none());
    }
}
