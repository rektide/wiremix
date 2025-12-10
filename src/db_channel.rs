//! Database persistence using MPSC channels for async communication.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;
use futures::executor::block_on;

use crate::wirehose::{
    state::{Client as WireClient, Device, Link, Metadata, Node},
    ObjectId,
};
use crate::db::Database;

/// Messages sent to the database thread
#[derive(Debug)]
pub enum DbMessage {
    // Client operations
    UpsertClient(WireClient),
    
    // Node operations  
    UpsertNode(Node),
    
    // Device operations
    UpsertDevice(Device),
    
    // Link operations
    UpsertLink { object_id: ObjectId, link: Link },
    
    // Metadata operations
    UpsertMetadata(Metadata),
    RemoveMetadataProperty { object_id: ObjectId, subject: u32, key: String },
    ClearMetadataProperties { object_id: ObjectId, subject: u32 },
    
    // Object removal
    RemoveObject(ObjectId),
    
    // Shutdown
    Shutdown,
}

/// Database handle that sends messages to the database thread
#[derive(Clone)]
pub struct DatabaseHandle {
    sender: Sender<DbMessage>,
}

impl DatabaseHandle {
    /// Send a message to the database thread
    pub fn send(&self, message: DbMessage) -> Result<()> {
        self.sender.send(message)?;
        Ok(())
    }
}

/// Database thread that processes messages and performs actual database operations
pub struct DatabaseThread {
    receiver: Receiver<DbMessage>,
    db: Database,
}

impl DatabaseThread {
    /// Create a new database thread
    pub fn new(database_url: &str) -> Result<(Self, DatabaseHandle)> {
        let db = block_on(Database::new(database_url))?;
        
        let (sender, receiver) = mpsc::channel();
        let handle = DatabaseHandle { sender };
        
        Ok((Self { receiver, db }, handle))
    }
    
    /// Run the database event loop
    pub fn run(self) {
        thread::spawn(move || {
            log::info!("Database thread started");
            
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            
            loop {
                match self.receiver.recv() {
                    Ok(message) => {
                        if let Err(e) = runtime.block_on(self.handle_message(message)) {
                            log::error!("Database operation failed: {}", e);
                        }
                    }
                    Err(mpsc::RecvError) => {
                        log::info!("Database channel closed, shutting down");
                        break;
                    }
                }
            }
            
            log::info!("Database thread stopped");
        });
    }
    
    async fn handle_message(&self, message: DbMessage) -> Result<()> {
        match message {
            DbMessage::UpsertClient(client) => self.db.upsert_client(&client).await,
            DbMessage::UpsertNode(node) => self.db.upsert_node(&node).await,
            DbMessage::UpsertDevice(device) => self.db.upsert_device(&device).await,
            DbMessage::UpsertLink { object_id, link } => self.db.upsert_link(object_id, &link).await,
            DbMessage::UpsertMetadata(metadata) => self.db.upsert_metadata(&metadata).await,
            DbMessage::RemoveMetadataProperty { object_id, subject, key } => {
                self.db.remove_metadata_property(object_id, subject, &key).await
            }
            DbMessage::ClearMetadataProperties { object_id, subject } => {
                self.db.clear_metadata_properties(object_id, subject).await
            }
            DbMessage::RemoveObject(object_id) => self.db.remove_object(object_id).await,
            DbMessage::Shutdown => {
                log::info!("Received shutdown signal");
                Ok(())
            }
        }
    }
}