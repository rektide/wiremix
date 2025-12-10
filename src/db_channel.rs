//! Database persistence using MPSC channels for async communication.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;
use futures::executor::block_on;
use turso::{Builder, Database as TursoDatabase, params};

use crate::wirehose::{
    state::{Client as WireClient, Device, EnumRoute, Link, Metadata, Node, Profile, Route},
    ObjectId,
};

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
    db: TursoDatabase,
}

impl DatabaseThread {
    /// Create a new database thread
    pub fn new(database_url: &str) -> Result<(Self, DatabaseHandle)> {
        let db = block_on(async {
            Builder::new_local(database_url).build().await
        })?;
        
        // Run migrations
        block_on(Self::run_migrations(&db))?;
        
        let (sender, receiver) = mpsc::channel();
        let handle = DatabaseHandle { sender };
        
        Ok((Self { receiver, db }, handle))
    }
    
    async fn run_migrations(db: &TursoDatabase) -> Result<()> {
        // Read migration file
        let up_sql = include_str!("../migrations/0001_initial_schema.up.sql");
        
        // Execute migration
        let conn = db.connect()?;
        for statement in up_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                conn.execute(trimmed, ()).await?;
            }
        }
        
        Ok(())
    }
    
    /// Run the database event loop
    pub fn run(self) {
        thread::spawn(move || {
            log::info!("Database thread started");
            
            loop {
                match self.receiver.recv() {
                    Ok(message) => {
                        if let Err(e) = self.handle_message(message) {
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
    
    fn handle_message(&self, message: DbMessage) -> Result<()> {
        match message {
            DbMessage::UpsertClient(client) => self.upsert_client(&client),
            DbMessage::UpsertNode(node) => self.upsert_node(&node),
            DbMessage::UpsertDevice(device) => self.upsert_device(&device),
            DbMessage::UpsertLink { object_id, link } => self.upsert_link(object_id, &link),
            DbMessage::UpsertMetadata(metadata) => self.upsert_metadata(&metadata),
            DbMessage::RemoveMetadataProperty { object_id, subject, key } => {
                self.remove_metadata_property(object_id, subject, &key)
            }
            DbMessage::ClearMetadataProperties { object_id, subject } => {
                self.clear_metadata_properties(object_id, subject)
            }
            DbMessage::RemoveObject(object_id) => self.remove_object(object_id),
            DbMessage::Shutdown => {
                log::info!("Received shutdown signal");
                Ok(())
            }
        }
    }
    
    fn upsert_client(&self, client: &WireClient) -> Result<()> {
        let conn = self.db.connect()?;
        
        // PropertyStore doesn't implement Serialize, so we'll store as debug string for now
        let props_debug = format!("{:?}", client.props);
        let object_id: u32 = client.object_id.into();
        
        block_on(conn.execute(
            r#"
            INSERT INTO clients (object_id, props_json, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                props_json = excluded.props_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, props_debug],
        ))?;

        Ok(())
    }

    fn upsert_node(&self, node: &Node) -> Result<()> {
        let conn = self.db.connect()?;
        
        let props_debug = format!("{:?}", node.props);
        let object_id: u32 = node.object_id.into();
        
        // Convert optional fields to JSON strings
        let volumes_json = node.volumes.as_ref().map(|v| {
            serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
        });
        let peaks_json = node.peaks.as_ref().map(|p| {
            serde_json::to_string(p).unwrap_or_else(|_| "[]".to_string())
        });
        let positions_json = node.positions.as_ref().map(|p| {
            serde_json::to_string(p).unwrap_or_else(|_| "[]".to_string())
        });

        block_on(conn.execute(
            r#"
            INSERT INTO nodes (object_id, props_json, volumes_json, mute, peaks_json, rate, positions_json, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                props_json = excluded.props_json,
                volumes_json = excluded.volumes_json,
                mute = excluded.mute,
                peaks_json = excluded.peaks_json,
                rate = excluded.rate,
                positions_json = excluded.positions_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                object_id,
                props_debug,
                volumes_json,
                node.mute,
                peaks_json,
                node.rate,
                positions_json,
            ],
        ))?;

        Ok(())
    }

    fn upsert_device(&self, device: &Device) -> Result<()> {
        let conn = self.db.connect()?;
        
        let props_debug = format!("{:?}", device.props);
        let object_id: u32 = device.object_id.into();
        
        block_on(conn.execute(
            r#"
            INSERT INTO devices (object_id, props_json, profile_index, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                props_json = excluded.props_json,
                profile_index = excluded.profile_index,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, props_debug, device.profile_index],
        ))?;

        // Update profiles
        for (index, profile) in &device.profiles {
            self.upsert_device_profile(device.object_id, *index, profile)?;
        }

        // Update routes
        for (device_id, route) in &device.routes {
            self.upsert_device_route(device.object_id, *device_id, route)?;
        }

        // Update enum routes
        for (index, enum_route) in &device.enum_routes {
            self.upsert_device_enum_route(device.object_id, *index, enum_route)?;
        }

        Ok(())
    }

    fn upsert_device_profile(&self, device_id: ObjectId, profile_index: i32, profile: &Profile) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let classes_json = serde_json::to_string(&profile.classes)
            .unwrap_or_else(|_| "[]".to_string());

        block_on(conn.execute(
            r#"
            INSERT INTO device_profiles (device_id, profile_index, description, available, classes_json)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(device_id, profile_index) DO UPDATE SET
                description = excluded.description,
                available = excluded.available,
                classes_json = excluded.classes_json
            "#,
            params![
                device_id_raw,
                profile_index,
                profile.description.clone(),
                profile.available,
                classes_json,
            ],
        ))?;

        Ok(())
    }

    fn upsert_device_route(&self, device_id: ObjectId, route_device: i32, route: &Route) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let profiles_json = serde_json::to_string(&route.profiles)
            .unwrap_or_else(|_| "[]".to_string());
        let volumes_json = serde_json::to_string(&route.volumes)
            .unwrap_or_else(|_| "[]".to_string());

        block_on(conn.execute(
            r#"
            INSERT INTO device_routes (device_id, route_index, route_device, profiles_json, description, available, volumes_json, mute)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(device_id, route_index) DO UPDATE SET
                route_device = excluded.route_device,
                profiles_json = excluded.profiles_json,
                description = excluded.description,
                available = excluded.available,
                volumes_json = excluded.volumes_json,
                mute = excluded.mute
            "#,
            params![
                device_id_raw,
                route.index,
                route_device,
                profiles_json,
                route.description.clone(),
                route.available,
                volumes_json,
                route.mute,
            ],
        ))?;

        Ok(())
    }

    fn upsert_device_enum_route(&self, device_id: ObjectId, enum_route_index: i32, enum_route: &EnumRoute) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let profiles_json = serde_json::to_string(&enum_route.profiles)
            .unwrap_or_else(|_| "[]".to_string());
        let devices_json = serde_json::to_string(&enum_route.devices)
            .unwrap_or_else(|_| "[]".to_string());

        block_on(conn.execute(
            r#"
            INSERT INTO device_enum_routes (device_id, enum_route_index, description, available, profiles_json, devices_json)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(device_id, enum_route_index) DO UPDATE SET
                description = excluded.description,
                available = excluded.available,
                profiles_json = excluded.profiles_json,
                devices_json = excluded.devices_json
            "#,
            params![
                device_id_raw,
                enum_route_index,
                enum_route.description.clone(),
                enum_route.available,
                profiles_json,
                devices_json,
            ],
        ))?;

        Ok(())
    }

    fn upsert_link(&self, object_id: ObjectId, link: &Link) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        let output_id: u32 = link.output_id.into();
        let input_id: u32 = link.input_id.into();
        
        block_on(conn.execute(
            r#"
            INSERT INTO links (object_id, output_id, input_id, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                output_id = excluded.output_id,
                input_id = excluded.input_id,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id_raw, output_id, input_id],
        ))?;

        Ok(())
    }

    fn upsert_metadata(&self, metadata: &Metadata) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id: u32 = metadata.object_id.into();
        
        block_on(conn.execute(
            r#"
            INSERT INTO metadata (object_id, metadata_name, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                metadata_name = excluded.metadata_name,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, metadata.metadata_name.clone()],
        ))?;

        // Update properties
        for (subject, properties) in &metadata.properties {
            for (key, value) in properties {
                block_on(conn.execute(
                    r#"
                    INSERT INTO metadata_properties (metadata_id, subject, key, value)
                    VALUES (?, ?, ?, ?)
                    ON CONFLICT(metadata_id, subject, key) DO UPDATE SET
                        value = excluded.value
                    "#,
                    params![object_id, subject, key.clone(), value.clone()],
                ))?;
            }
        }

        Ok(())
    }

    fn remove_metadata_property(&self, object_id: ObjectId, subject: u32, key: &str) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        block_on(conn.execute(
            "DELETE FROM metadata_properties WHERE metadata_id = ? AND subject = ? AND key = ?",
            params![object_id_raw, subject, key],
        ))?;

        Ok(())
    }

    fn clear_metadata_properties(&self, object_id: ObjectId, subject: u32) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        block_on(conn.execute(
            "DELETE FROM metadata_properties WHERE metadata_id = ? AND subject = ?",
            params![object_id_raw, subject],
        ))?;

        Ok(())
    }

    fn remove_object(&self, object_id: ObjectId) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        // Try to delete from each table (cascading foreign keys will handle related records)
        let _ = block_on(conn.execute("DELETE FROM clients WHERE object_id = ?", params![object_id_raw]));
        let _ = block_on(conn.execute("DELETE FROM nodes WHERE object_id = ?", params![object_id_raw]));
        let _ = block_on(conn.execute("DELETE FROM devices WHERE object_id = ?", params![object_id_raw]));
        let _ = block_on(conn.execute("DELETE FROM links WHERE object_id = ?", params![object_id_raw]));
        let _ = block_on(conn.execute("DELETE FROM metadata WHERE object_id = ?", params![object_id_raw]));

        Ok(())
    }
}