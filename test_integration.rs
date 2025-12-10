use tempfile::NamedTempFile;
use wiremix::db::Database;

#[test]
fn test_database_creation() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    // Test creating a database
    let db = Database::new(db_path);
    assert!(db.is_ok(), "Failed to create database: {:?}", db.err());
    
    println!("Database created successfully at {}", db_path);
}

fn main() {
    test_database_creation();
    println!("Integration test passed!");
}
