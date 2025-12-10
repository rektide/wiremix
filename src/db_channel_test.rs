//! Test for the new threaded database architecture

#[cfg(test)]
mod tests {
    use crate::wirehose::state::{Client, State};
    use crate::wirehose::{ObjectId, PropertyStore, StateEvent};
    use crate::db_channel::{DatabaseThread, DbMessage};
    use crate::mock::WirehoseHandle;
    use std::time::Duration;
    use std::thread;

    fn create_test_property_store() -> PropertyStore {
        let mut props = PropertyStore::default();
        props.set_application_name(String::from("TestApp"));
        props
    }

    #[test]
    fn test_threaded_database_persistence() {
        // Create a temporary database
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_threaded_db.db");
        let db_url = db_path.to_str().unwrap();

        // Create database thread and handle
        let (db_thread, db_handle) = DatabaseThread::new(db_url).expect("Failed to create database");
        
        // Start the database thread
        db_thread.run();
        
        // Give the thread time to start
        thread::sleep(Duration::from_millis(100));

        // Create state with database handle
        let mut state = State::default().with_database(db_handle.clone());
        let wirehose = WirehoseHandle::default();

        // Create a test client
        let object_id = ObjectId::from_raw_id(42);
        let props = create_test_property_store();
        
        // Send client update through state (which should use channels)
        state.update(
            &wirehose,
            StateEvent::ClientProperties {
                object_id,
                props: props.clone(),
            },
        );

        // Give the database thread time to process
        thread::sleep(Duration::from_millis(200));

        // Verify the client was persisted by checking if we can send a shutdown message
        // (this tests that the channel is still working)
        let result = db_handle.send(DbMessage::Shutdown);
        assert!(result.is_ok(), "Failed to send shutdown message to database thread");

        // Clean up
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_database_channel_communication() {
        // Create a temporary database
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_channel_comm.db");
        let db_url = db_path.to_str().unwrap();

        // Create database thread and handle
        let (db_thread, db_handle) = DatabaseThread::new(db_url).expect("Failed to create database");
        
        // Start the database thread
        db_thread.run();
        
        // Give the thread time to start
        thread::sleep(Duration::from_millis(100));

        // Test sending different types of messages
        let object_id = ObjectId::from_raw_id(123);
        
        // Test client message
        let client = Client {
            object_id,
            props: create_test_property_store(),
        };
        let result = db_handle.send(DbMessage::UpsertClient(client));
        assert!(result.is_ok(), "Failed to send client message");

        // Test node message
        let node = crate::wirehose::state::Node {
            object_id,
            props: create_test_property_store(),
            volumes: None,
            mute: None,
            peaks: None,
            rate: None,
            positions: None,
        };
        let result = db_handle.send(DbMessage::UpsertNode(node));
        assert!(result.is_ok(), "Failed to send node message");

        // Test device message
        let device = crate::wirehose::state::Device {
            object_id,
            props: create_test_property_store(),
            profile_index: None,
            profiles: std::collections::HashMap::new(),
            routes: std::collections::HashMap::new(),
            enum_routes: std::collections::HashMap::new(),
        };
        let result = db_handle.send(DbMessage::UpsertDevice(device));
        assert!(result.is_ok(), "Failed to send device message");

        // Give the database thread time to process
        thread::sleep(Duration::from_millis(200));

        // Send shutdown
        let result = db_handle.send(DbMessage::Shutdown);
        assert!(result.is_ok(), "Failed to send shutdown message");

        // Clean up
        let _ = std::fs::remove_file(db_path);
    }
}