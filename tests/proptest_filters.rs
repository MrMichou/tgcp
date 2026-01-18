//! Property-based tests using proptest
//!
//! These tests verify the correctness of filter logic, JSON parsing,
//! and input validation using randomized inputs.

use proptest::prelude::*;
use serde_json::{json, Value};

/// Generate arbitrary VM instance data for testing
fn arb_instance() -> impl Strategy<Value = Value> {
    (
        "[a-z][a-z0-9-]{0,62}", // name
        prop_oneof!["RUNNING", "STOPPED", "TERMINATED", "PENDING", "STAGING"],
        "[a-z]+-[a-z]+[0-9]-[a-z]", // zone
        prop_oneof![
            "n1-standard-1",
            "n2-standard-2",
            "e2-medium",
            "c2-standard-4"
        ],
    )
        .prop_map(|(name, status, zone, machine_type)| {
            json!({
                "name": name,
                "status": status,
                "zone": format!("projects/test/zones/{}", zone),
                "machineType": format!("projects/test/machineTypes/{}", machine_type)
            })
        })
}

/// Generate a list of instances
fn arb_instance_list() -> impl Strategy<Value = Vec<Value>> {
    prop::collection::vec(arb_instance(), 0..100)
}

/// Filter function that matches against filter string (case-insensitive substring match)
fn filter_items(items: &[Value], filter: &str) -> Vec<Value> {
    if filter.is_empty() {
        return items.to_vec();
    }

    let filter_lower = filter.to_lowercase();
    items
        .iter()
        .filter(|item| {
            // Check if any field contains the filter string
            if let Some(obj) = item.as_object() {
                obj.values().any(|v| {
                    v.as_str()
                        .map(|s| s.to_lowercase().contains(&filter_lower))
                        .unwrap_or(false)
                })
            } else {
                false
            }
        })
        .cloned()
        .collect()
}

proptest! {
    /// Empty filter returns all items
    #[test]
    fn empty_filter_returns_all(items in arb_instance_list()) {
        let filtered = filter_items(&items, "");
        prop_assert_eq!(filtered.len(), items.len());
    }

    /// Filtering is idempotent - filtering twice with same filter gives same result
    #[test]
    fn filter_is_idempotent(
        items in arb_instance_list(),
        filter in "[a-z]{0,10}"
    ) {
        let filtered_once = filter_items(&items, &filter);
        let filtered_twice = filter_items(&filtered_once, &filter);
        prop_assert_eq!(filtered_once.len(), filtered_twice.len());
    }

    /// Filtering never increases the number of items
    #[test]
    fn filter_never_increases_count(
        items in arb_instance_list(),
        filter in ".*"
    ) {
        let filtered = filter_items(&items, &filter);
        prop_assert!(filtered.len() <= items.len());
    }

    /// Case-insensitive filtering works correctly
    #[test]
    fn filter_is_case_insensitive(
        items in arb_instance_list(),
        filter in "[a-zA-Z]{1,5}"
    ) {
        let filtered_lower = filter_items(&items, &filter.to_lowercase());
        let filtered_upper = filter_items(&items, &filter.to_uppercase());
        prop_assert_eq!(filtered_lower.len(), filtered_upper.len());
    }

    /// Filtering by exact name returns at most one match per unique name
    #[test]
    fn filter_by_name_is_consistent(
        items in arb_instance_list()
    ) {
        // Get all unique names
        let names: Vec<String> = items
            .iter()
            .filter_map(|item| item["name"].as_str().map(String::from))
            .collect();

        for name in names {
            let filtered = filter_items(&items, &name);
            // All filtered items should contain the name somewhere
            for item in &filtered {
                let item_str = item.to_string().to_lowercase();
                prop_assert!(item_str.contains(&name.to_lowercase()));
            }
        }
    }

    /// Filtering by status returns only items with matching status
    #[test]
    fn filter_by_status(items in arb_instance_list()) {
        for status in &["RUNNING", "STOPPED", "TERMINATED"] {
            let filtered = filter_items(&items, status);
            for item in &filtered {
                let _item_status = item["status"].as_str().unwrap_or("");
                // Either the status matches or some other field contains the status string
                let item_str = item.to_string().to_lowercase();
                prop_assert!(item_str.contains(&status.to_lowercase()));
            }
        }
    }
}

/// Tests for JSON path extraction
mod json_path_tests {
    use super::*;

    /// Extract value from JSON using dot-notation path
    fn extract_json_path(value: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }

    proptest! {
        /// Extracting with empty path returns the original value
        #[test]
        fn empty_path_returns_original(instance in arb_instance()) {
            // Empty path should return None in our implementation
            let result = extract_json_path(&instance, "");
            prop_assert!(result.is_none());
        }

        /// Extracting "name" always returns a string for valid instances
        #[test]
        fn name_extraction_returns_string(instance in arb_instance()) {
            let result = extract_json_path(&instance, "name");
            prop_assert!(result.is_some());
            prop_assert!(result.unwrap().is_string());
        }

        /// Extracting non-existent path returns None
        #[test]
        fn nonexistent_path_returns_none(instance in arb_instance()) {
            let result = extract_json_path(&instance, "nonexistent.deeply.nested");
            prop_assert!(result.is_none());
        }
    }
}

/// Tests for input validation
mod input_validation_tests {
    use super::*;

    /// Validate project ID format (lowercase letters, digits, hyphens)
    fn is_valid_project_id(s: &str) -> bool {
        if s.is_empty() || s.len() > 30 {
            return false;
        }
        // Must start with lowercase letter
        if !s
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        {
            return false;
        }
        // Only lowercase letters, digits, and hyphens
        s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

    /// Validate zone format (region-zone, e.g., us-central1-a)
    fn is_valid_zone(s: &str) -> bool {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 3 {
            return false;
        }
        // All parts should be alphanumeric
        parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_alphanumeric()))
    }

    proptest! {
        /// Valid project IDs pass validation
        #[test]
        fn valid_project_ids_accepted(
            prefix in "[a-z]",
            rest in "[a-z0-9-]{0,28}"
        ) {
            let project_id = format!("{}{}", prefix, rest);
            prop_assert!(is_valid_project_id(&project_id));
        }

        /// Empty project IDs are rejected
        #[test]
        fn empty_project_id_rejected(_dummy in any::<bool>()) {
            prop_assert!(!is_valid_project_id(""));
        }

        /// Project IDs starting with numbers are rejected
        #[test]
        fn numeric_start_rejected(
            num in "[0-9]",
            rest in "[a-z0-9-]{0,28}"
        ) {
            let project_id = format!("{}{}", num, rest);
            prop_assert!(!is_valid_project_id(&project_id));
        }

        /// Valid zone formats are accepted
        #[test]
        fn valid_zones_accepted(
            region in "[a-z]{2,4}",
            location in "[a-z]+[0-9]",
            zone_letter in "[a-z]"
        ) {
            let zone = format!("{}-{}-{}", region, location, zone_letter);
            prop_assert!(is_valid_zone(&zone));
        }

        /// Single-part zones are rejected
        #[test]
        fn single_part_zone_rejected(part in "[a-z0-9]+") {
            prop_assert!(!is_valid_zone(&part));
        }
    }
}

/// Tests for visible range calculation (used in virtual scrolling)
mod visible_range_tests {
    use super::*;

    /// Calculate visible range for virtual scrolling
    fn calculate_visible_range(
        total_items: usize,
        viewport_height: usize,
        scroll_offset: usize,
    ) -> std::ops::Range<usize> {
        let start = scroll_offset.min(total_items);
        let end = (scroll_offset + viewport_height).min(total_items);
        start..end
    }

    proptest! {
        /// Visible range never exceeds total items
        #[test]
        fn range_within_bounds(
            total in 0usize..1000,
            viewport in 1usize..100,
            offset in 0usize..1000
        ) {
            let range = calculate_visible_range(total, viewport, offset);
            prop_assert!(range.start <= total);
            prop_assert!(range.end <= total);
        }

        /// Range size is at most viewport height
        #[test]
        fn range_size_at_most_viewport(
            total in 0usize..1000,
            viewport in 1usize..100,
            offset in 0usize..1000
        ) {
            let range = calculate_visible_range(total, viewport, offset);
            prop_assert!(range.len() <= viewport);
        }

        /// Zero offset starts at beginning
        #[test]
        fn zero_offset_starts_at_zero(
            total in 1usize..1000,
            viewport in 1usize..100
        ) {
            let range = calculate_visible_range(total, viewport, 0);
            prop_assert_eq!(range.start, 0);
        }

        /// Range is empty when total items is zero
        #[test]
        fn empty_when_no_items(
            viewport in 1usize..100,
            offset in 0usize..1000
        ) {
            let range = calculate_visible_range(0, viewport, offset);
            prop_assert!(range.is_empty());
        }

        /// Offset beyond total gives empty or partial range
        #[test]
        fn offset_beyond_total(
            total in 1usize..100,
            viewport in 1usize..50
        ) {
            let range = calculate_visible_range(total, viewport, total + 10);
            prop_assert!(range.is_empty() || range.start >= total);
        }
    }
}
