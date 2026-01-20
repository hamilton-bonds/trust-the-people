/// Hierarchical jurisdiction system for US voting with exact location tracking
///
/// Supports the full US voting hierarchy:
/// - Federal (US)
/// - State (50 states + DC)
/// - County/Parish/Borough
/// - City/Township/Municipality
/// - Tribal Nations (sovereign)
/// - Precinct/Ward/District
/// - Polling Place (exact physical location with address and GPS)
///
/// Enables anomaly detection at any level, including:
/// - Specific polling place anomalies (address-level)
/// - Precinct-level irregularities
/// - County/city aggregates
/// - State-wide patterns
/// - Federal oversight

use common::{ElectionId, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Jurisdiction hierarchy level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JurisdictionLevel {
    /// Federal level (US)
    Federal,
    /// State level (including DC, territories)
    State,
    /// County, Parish, or Borough
    County,
    /// City, Township, or Municipality
    City,
    /// Tribal Nation (sovereign jurisdiction)
    Tribal,
    /// Precinct, Ward, or District (smallest voting unit)
    Precinct,
    /// Individual polling place (exact physical location)
    PollingPlace,
}

impl JurisdictionLevel {
    pub fn parent_level(&self) -> Option<Self> {
        match self {
            JurisdictionLevel::Federal => None,
            JurisdictionLevel::State => Some(JurisdictionLevel::Federal),
            JurisdictionLevel::County => Some(JurisdictionLevel::State),
            JurisdictionLevel::City => Some(JurisdictionLevel::County),
            JurisdictionLevel::Tribal => Some(JurisdictionLevel::Federal),
            JurisdictionLevel::Precinct => Some(JurisdictionLevel::City),
            JurisdictionLevel::PollingPlace => Some(JurisdictionLevel::Precinct),
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            JurisdictionLevel::Federal => "federal",
            JurisdictionLevel::State => "state",
            JurisdictionLevel::County => "county",
            JurisdictionLevel::City => "city",
            JurisdictionLevel::Tribal => "tribal",
            JurisdictionLevel::Precinct => "precinct",
            JurisdictionLevel::PollingPlace => "polling_place",
        }
    }
}

impl std::fmt::Display for JurisdictionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

/// Jurisdiction identifier and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jurisdiction {
    /// Unique identifier (e.g., "US", "CA", "CA-LA", "CA-LA-001")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Jurisdiction level
    pub level: JurisdictionLevel,

    /// Parent jurisdiction ID
    pub parent_id: Option<String>,

    /// FIPS code (for counties/states)
    pub fips_code: Option<String>,

    /// State code (e.g., "CA", "TX")
    pub state_code: Option<String>,

    /// County code
    pub county_code: Option<String>,

    /// Precinct/district number
    pub precinct_number: Option<String>,

    /// Associated voting locations
    pub voting_locations: Vec<VotingLocation>,

    /// Metadata
    pub metadata: JurisdictionMetadata,
}

impl Jurisdiction {
    pub fn new(id: String, name: String, level: JurisdictionLevel) -> Self {
        Self {
            id,
            name,
            level,
            parent_id: None,
            fips_code: None,
            state_code: None,
            county_code: None,
            precinct_number: None,
            voting_locations: Vec::new(),
            metadata: JurisdictionMetadata::default(),
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_fips(mut self, fips: String) -> Self {
        self.fips_code = Some(fips);
        self
    }

    pub fn with_state_code(mut self, code: String) -> Self {
        self.state_code = Some(code);
        self
    }

    pub fn add_voting_location(&mut self, location: VotingLocation) {
        self.voting_locations.push(location);
    }

    pub fn location_count(&self) -> usize {
        self.voting_locations.len()
    }

    pub fn get_location(&self, location_id: &str) -> Option<&VotingLocation> {
        self.voting_locations.iter().find(|loc| loc.id == location_id)
    }
}

/// Physical voting location with exact address and GPS coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingLocation {
    /// Unique location identifier
    pub id: String,

    /// Location name (e.g., "Lincoln Elementary School")
    pub name: String,

    /// Street address
    pub address: Address,

    /// GPS coordinates for mapping and anomaly detection
    pub coordinates: GeoCoordinates,

    /// Location type
    pub location_type: LocationType,

    /// Metadata about this location
    pub metadata: LocationMetadata,

    /// Voting statistics for this exact location
    pub stats: LocationStats,
}

impl VotingLocation {
    pub fn new(
        id: String,
        name: String,
        address: Address,
        coordinates: GeoCoordinates,
    ) -> Self {
        Self {
            id,
            name,
            address,
            coordinates,
            location_type: LocationType::PollingPlace,
            metadata: LocationMetadata::default(),
            stats: LocationStats::default(),
        }
    }

    /// Calculate distance to another location (in meters)
    pub fn distance_to(&self, other: &VotingLocation) -> f64 {
        self.coordinates.distance_to(&other.coordinates)
    }
}

/// Physical address structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub street_address: String,
    pub street_address_2: Option<String>,
    pub city: String,
    pub state: String,
    pub zip_code: String,
    pub county: Option<String>,
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}, {}, {} {}",
            self.street_address, self.city, self.state, self.zip_code
        )
    }
}

/// GPS coordinates for exact location tracking
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeoCoordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

impl GeoCoordinates {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            altitude: None,
        }
    }

    /// Calculate distance using Haversine formula (in meters)
    pub fn distance_to(&self, other: &GeoCoordinates) -> f64 {
        let earth_radius = 6371000.0; // meters

        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        earth_radius * c
    }
}

/// Type of voting location
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LocationType {
    /// Standard polling place
    PollingPlace,
    /// Early voting center
    EarlyVotingCenter,
    /// Vote by mail drop box
    DropBox,
    /// Mobile voting unit
    MobileUnit,
    /// Election office
    ElectionOffice,
}

/// Location metadata for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationMetadata {
    /// Registered voters assigned to this location
    pub registered_voters: u64,

    /// Accessibility features
    pub is_accessible: bool,

    /// Hours of operation
    pub hours: Option<String>,

    /// Contact information
    pub contact_phone: Option<String>,

    /// Additional notes
    pub notes: Option<String>,
}

impl Default for LocationMetadata {
    fn default() -> Self {
        Self {
            registered_voters: 0,
            is_accessible: true,
            hours: None,
            contact_phone: None,
            notes: None,
        }
    }
}

/// Statistics for a specific voting location (for anomaly detection)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocationStats {
    /// Total votes cast at this location
    pub votes_cast: u64,

    /// Turnout percentage
    pub turnout_percentage: f64,

    /// Average wait time (minutes)
    pub avg_wait_time: Option<u32>,

    /// Peak voting hour
    pub peak_hour: Option<u32>,

    /// Anomaly flags
    pub anomaly_flags: Vec<String>,

    /// Last updated
    pub last_updated: Timestamp,
}

/// Jurisdiction metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JurisdictionMetadata {
    pub population: Option<u64>,
    pub registered_voters: u64,
    pub area_sq_miles: Option<f64>,
    pub timezone: Option<String>,
}

/// Jurisdiction path for hierarchical queries
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JurisdictionPath {
    components: Vec<String>,
}

impl JurisdictionPath {
    pub fn new(components: Vec<String>) -> Self {
        Self { components }
    }

    /// Parse from string (e.g., "US/California/Los Angeles/Los Angeles/001/Location-123")
    pub fn parse(path: &str) -> Result<Self> {
        let components: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if components.is_empty() {
            return Err(VotingError::InvalidInput(
                "Empty jurisdiction path".to_string(),
            ));
        }

        Ok(Self { components })
    }

    pub fn to_string(&self) -> String {
        self.components.join("/")
    }

    pub fn depth(&self) -> usize {
        self.components.len()
    }

    pub fn parent(&self) -> Option<Self> {
        if self.components.len() <= 1 {
            return None;
        }

        Some(Self {
            components: self.components[..self.components.len() - 1].to_vec(),
        })
    }

    pub fn child(&self, component: String) -> Self {
        let mut components = self.components.clone();
        components.push(component);
        Self { components }
    }

    pub fn is_ancestor_of(&self, other: &JurisdictionPath) -> bool {
        if self.depth() >= other.depth() {
            return false;
        }

        self.components
            .iter()
            .zip(other.components.iter())
            .all(|(a, b)| a == b)
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }
}

impl std::fmt::Display for JurisdictionPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Jurisdiction tree for managing hierarchy
pub struct JurisdictionTree {
    jurisdictions: HashMap<String, Jurisdiction>,
    path_index: HashMap<JurisdictionPath, String>,
}

impl JurisdictionTree {
    pub fn new() -> Self {
        Self {
            jurisdictions: HashMap::new(),
            path_index: HashMap::new(),
        }
    }

    pub fn add_jurisdiction(&mut self, jurisdiction: Jurisdiction) -> Result<()> {
        let id = jurisdiction.id.clone();
        
        if self.jurisdictions.contains_key(&id) {
            return Err(VotingError::InvalidInput(
                "Jurisdiction already exists".to_string(),
            ));
        }

        let path = self.build_path(&jurisdiction)?;
        self.path_index.insert(path, id.clone());
        self.jurisdictions.insert(id, jurisdiction);

        Ok(())
    }

    pub fn get_jurisdiction(&self, id: &str) -> Option<&Jurisdiction> {
        self.jurisdictions.get(id)
    }

    pub fn get_by_path(&self, path: &JurisdictionPath) -> Option<&Jurisdiction> {
        self.path_index
            .get(path)
            .and_then(|id| self.jurisdictions.get(id))
    }

    pub fn get_children(&self, parent_id: &str) -> Vec<&Jurisdiction> {
        self.jurisdictions
            .values()
            .filter(|j| j.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    pub fn get_all_descendants(&self, parent_id: &str) -> Vec<&Jurisdiction> {
        let mut descendants = Vec::new();
        let mut queue = vec![parent_id];

        while let Some(id) = queue.pop() {
            for child in self.get_children(id) {
                descendants.push(child);
                queue.push(&child.id);
            }
        }

        descendants
    }

    /// Get all voting locations in a jurisdiction and its descendants
    pub fn get_all_locations(&self, jurisdiction_id: &str) -> Vec<&VotingLocation> {
        let mut locations = Vec::new();

        if let Some(jurisdiction) = self.get_jurisdiction(jurisdiction_id) {
            locations.extend(jurisdiction.voting_locations.iter());
        }

        for descendant in self.get_all_descendants(jurisdiction_id) {
            locations.extend(descendant.voting_locations.iter());
        }

        locations
    }

    /// Find voting location by address (fuzzy search)
    pub fn find_location_by_address(&self, address: &str) -> Vec<&VotingLocation> {
        let search_lower = address.to_lowercase();
        let mut results = Vec::new();

        for jurisdiction in self.jurisdictions.values() {
            for location in &jurisdiction.voting_locations {
                let loc_address = location.address.to_string().to_lowercase();
                if loc_address.contains(&search_lower) {
                    results.push(location);
                }
            }
        }

        results
    }

    /// Find voting locations near GPS coordinates (within radius in meters)
    pub fn find_locations_near(&self, coords: &GeoCoordinates, radius_meters: f64) -> Vec<&VotingLocation> {
        let mut results = Vec::new();

        for jurisdiction in self.jurisdictions.values() {
            for location in &jurisdiction.voting_locations {
                if location.coordinates.distance_to(coords) <= radius_meters {
                    results.push(location);
                }
            }
        }

        results
    }

    fn build_path(&self, jurisdiction: &Jurisdiction) -> Result<JurisdictionPath> {
        let mut components = Vec::new();
        let mut current_id = Some(jurisdiction.id.clone());

        while let Some(id) = current_id {
            components.insert(0, id.clone());
            current_id = self
                .jurisdictions
                .get(&id)
                .and_then(|j| j.parent_id.clone());
        }

        Ok(JurisdictionPath::new(components))
    }

    pub fn jurisdiction_count(&self) -> usize {
        self.jurisdictions.len()
    }

    pub fn total_locations(&self) -> usize {
        self.jurisdictions
            .values()
            .map(|j| j.voting_locations.len())
            .sum()
    }
}

impl Default for JurisdictionTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jurisdiction_levels() {
        assert_eq!(JurisdictionLevel::State.parent_level(), Some(JurisdictionLevel::Federal));
        assert_eq!(JurisdictionLevel::Federal.parent_level(), None);
    }

    #[test]
    fn test_jurisdiction_path_parse() {
        let path = JurisdictionPath::parse("US/California/Los Angeles").unwrap();
        assert_eq!(path.depth(), 3);
        assert_eq!(path.to_string(), "US/California/Los Angeles");
    }

    #[test]
    fn test_jurisdiction_path_parent() {
        let path = JurisdictionPath::parse("US/California/Los Angeles").unwrap();
        let parent = path.parent().unwrap();
        assert_eq!(parent.to_string(), "US/California");
    }

    #[test]
    fn test_jurisdiction_path_child() {
        let path = JurisdictionPath::parse("US/California").unwrap();
        let child = path.child("Los Angeles".to_string());
        assert_eq!(child.to_string(), "US/California/Los Angeles");
    }

    #[test]
    fn test_jurisdiction_path_ancestry() {
        let parent = JurisdictionPath::parse("US/California").unwrap();
        let child = JurisdictionPath::parse("US/California/Los Angeles").unwrap();

        assert!(parent.is_ancestor_of(&child));
        assert!(!child.is_ancestor_of(&parent));
    }

    #[test]
    fn test_geo_coordinates_distance() {
        let la = GeoCoordinates::new(34.0522, -118.2437);
        let sf = GeoCoordinates::new(37.7749, -122.4194);

        let distance = la.distance_to(&sf);
        assert!(distance > 500000.0 && distance < 600000.0);
    }

    #[test]
    fn test_voting_location_creation() {
        let address = Address {
            street_address: "123 Main St".to_string(),
            street_address_2: None,
            city: "Los Angeles".to_string(),
            state: "CA".to_string(),
            zip_code: "90001".to_string(),
            county: Some("Los Angeles".to_string()),
        };

        let coords = GeoCoordinates::new(34.0522, -118.2437);
        let location = VotingLocation::new(
            "loc-001".to_string(),
            "City Hall".to_string(),
            address,
            coords,
        );

        assert_eq!(location.id, "loc-001");
        assert_eq!(location.name, "City Hall");
    }

    #[test]
    fn test_jurisdiction_tree() {
        let mut tree = JurisdictionTree::new();

        let us = Jurisdiction::new("US".to_string(), "United States".to_string(), JurisdictionLevel::Federal);
        tree.add_jurisdiction(us).unwrap();

        let ca = Jurisdiction::new("CA".to_string(), "California".to_string(), JurisdictionLevel::State)
            .with_parent("US".to_string());
        tree.add_jurisdiction(ca).unwrap();

        assert_eq!(tree.jurisdiction_count(), 2);
        assert_eq!(tree.get_children("US").len(), 1);
    }

    #[test]
    fn test_address_display() {
        let address = Address {
            street_address: "123 Main St".to_string(),
            street_address_2: None,
            city: "Los Angeles".to_string(),
            state: "CA".to_string(),
            zip_code: "90001".to_string(),
            county: None,
        };

        assert_eq!(address.to_string(), "123 Main St, Los Angeles, CA 90001");
    }

    #[test]
    fn test_find_locations_near() {
        let mut tree = JurisdictionTree::new();

        let mut jurisdiction = Jurisdiction::new("test".to_string(), "Test".to_string(), JurisdictionLevel::City);
        
        let address = Address {
            street_address: "123 Main St".to_string(),
            street_address_2: None,
            city: "Test City".to_string(),
            state: "CA".to_string(),
            zip_code: "90001".to_string(),
            county: None,
        };

        let coords = GeoCoordinates::new(34.0522, -118.2437);
        let location = VotingLocation::new("loc-001".to_string(), "Test Location".to_string(), address, coords);
        
        jurisdiction.add_voting_location(location);
        tree.add_jurisdiction(jurisdiction).unwrap();

        let search_coords = GeoCoordinates::new(34.0522, -118.2437);
        let nearby = tree.find_locations_near(&search_coords, 100.0);

        assert_eq!(nearby.len(), 1);
    }
}
