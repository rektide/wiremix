//! Database persistence for PipeWire state.

use anyhow::Result;
use turso::{Database as TursoDatabase, params};

use crate::wirehose::{
    state::{Client as WireClient, Device, EnumRoute, Link, Metadata, Node, Profile, Route},
    ObjectId,
};

/// Database operations that can be called from within an async context
pub struct Database {
    db: TursoDatabase,
}

impl Database {
    /// Create a new database connection (this should be called from within an async context)
    pub async fn new(database_url: &str) -> Result<Self> {
        let db = turso::Builder::new_local(database_url).build().await?;
        
        // Run migrations
        Self::run_migrations(&db).await?;

        Ok(Self { db })
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

    /// Insert or update a client.
    pub async fn upsert_client(&self, client: &WireClient) -> Result<()> {
        let conn = self.db.connect()?;
        
        // PropertyStore doesn't implement Serialize, so we'll store as debug string for now
        let props_debug = format!("{:?}", client.props);
        let object_id: u32 = client.object_id.into();
        
        conn.execute(
            r#"
            INSERT INTO clients (object_id, props_json, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                props_json = excluded.props_json,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, props_debug],
        ).await?;

        Ok(())
    }

    /// Insert or update a node.
    pub async fn upsert_node(&self, node: &Node) -> Result<()> {
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

        conn.execute(
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
        ).await?;

        Ok(())
    }

    /// Insert or update a device.
    pub async fn upsert_device(&self, device: &Device) -> Result<()> {
        let conn = self.db.connect()?;
        
        let props_debug = format!("{:?}", device.props);
        let object_id: u32 = device.object_id.into();
        
        conn.execute(
            r#"
            INSERT INTO devices (object_id, props_json, profile_index, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                props_json = excluded.props_json,
                profile_index = excluded.profile_index,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, props_debug, device.profile_index],
        ).await?;

        // Update profiles
        for (index, profile) in &device.profiles {
            self.upsert_device_profile(device.object_id, *index, profile).await?;
        }

        // Update routes
        for (device_id, route) in &device.routes {
            self.upsert_device_route(device.object_id, *device_id, route).await?;
        }

        // Update enum routes
        for (index, enum_route) in &device.enum_routes {
            self.upsert_device_enum_route(device.object_id, *index, enum_route).await?;
        }

        Ok(())
    }

    /// Insert or update a device profile.
    async fn upsert_device_profile(&self, device_id: ObjectId, profile_index: i32, profile: &Profile) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let classes_json = serde_json::to_string(&profile.classes)
            .unwrap_or_else(|_| "[]".to_string());

        conn.execute(
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
        ).await?;

        Ok(())
    }

    /// Insert or update a device route.
    async fn upsert_device_route(&self, device_id: ObjectId, route_device: i32, route: &Route) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let profiles_json = serde_json::to_string(&route.profiles)
            .unwrap_or_else(|_| "[]".to_string());
        let volumes_json = serde_json::to_string(&route.volumes)
            .unwrap_or_else(|_| "[]".to_string());

        conn.execute(
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
        ).await?;

        Ok(())
    }

    /// Insert or update a device enum route.
    async fn upsert_device_enum_route(&self, device_id: ObjectId, enum_route_index: i32, enum_route: &EnumRoute) -> Result<()> {
        let conn = self.db.connect()?;
        
        let device_id_raw: u32 = device_id.into();
        let profiles_json = serde_json::to_string(&enum_route.profiles)
            .unwrap_or_else(|_| "[]".to_string());
        let devices_json = serde_json::to_string(&enum_route.devices)
            .unwrap_or_else(|_| "[]".to_string());

        conn.execute(
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
        ).await?;

        Ok(())
    }

    /// Insert or update a link.
    pub async fn upsert_link(&self, object_id: ObjectId, link: &Link) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        let output_id: u32 = link.output_id.into();
        let input_id: u32 = link.input_id.into();
        
        conn.execute(
            r#"
            INSERT INTO links (object_id, output_id, input_id, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                output_id = excluded.output_id,
                input_id = excluded.input_id,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id_raw, output_id, input_id],
        ).await?;

        Ok(())
    }

    /// Insert or update metadata.
    pub async fn upsert_metadata(&self, metadata: &Metadata) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id: u32 = metadata.object_id.into();
        
        conn.execute(
            r#"
            INSERT INTO metadata (object_id, metadata_name, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(object_id) DO UPDATE SET
                metadata_name = excluded.metadata_name,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![object_id, metadata.metadata_name.clone()],
        ).await?;

        // Update properties
        for (subject, properties) in &metadata.properties {
            for (key, value) in properties {
                conn.execute(
                    r#"
                    INSERT INTO metadata_properties (metadata_id, subject, key, value)
                    VALUES (?, ?, ?, ?)
                    ON CONFLICT(metadata_id, subject, key) DO UPDATE SET
                        value = excluded.value
                    "#,
                    params![object_id, subject, key.clone(), value.clone()],
                ).await?;
            }
        }

        Ok(())
    }

    /// Remove a specific metadata property.
    pub async fn remove_metadata_property(&self, object_id: ObjectId, subject: u32, key: &str) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        conn.execute(
            "DELETE FROM metadata_properties WHERE metadata_id = ? AND subject = ? AND key = ?",
            params![object_id_raw, subject, key],
        ).await?;

        Ok(())
    }

    /// Clear all properties for a metadata subject.
    pub async fn clear_metadata_properties(&self, object_id: ObjectId, subject: u32) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        conn.execute(
            "DELETE FROM metadata_properties WHERE metadata_id = ? AND subject = ?",
            params![object_id_raw, subject],
        ).await?;

        Ok(())
    }

    /// Remove an object from the database.
    pub async fn remove_object(&self, object_id: ObjectId) -> Result<()> {
        let conn = self.db.connect()?;
        
        let object_id_raw: u32 = object_id.into();
        
        // Try to delete from each table (cascading foreign keys will handle related records)
        let _ = conn.execute("DELETE FROM clients WHERE object_id = ?", params![object_id_raw]).await;
        let _ = conn.execute("DELETE FROM nodes WHERE object_id = ?", params![object_id_raw]).await;
        let _ = conn.execute("DELETE FROM devices WHERE object_id = ?", params![object_id_raw]).await;
        let _ = conn.execute("DELETE FROM links WHERE object_id = ?", params![object_id_raw]).await;
        let _ = conn.execute("DELETE FROM metadata WHERE object_id = ?", params![object_id_raw]).await;

        Ok(())
    }
}