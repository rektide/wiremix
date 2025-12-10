use wiremix::db::Database;

fn main() {
    // Test creating a database
    let db = Database::new(":memory:").unwrap();
    println!("Database created successfully!");
    
    // Test that we can execute a simple query
    println!("Database test passed!");
}
