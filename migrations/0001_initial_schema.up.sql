-- Create clients table
CREATE TABLE IF NOT EXISTS clients (
    object_id INTEGER PRIMARY KEY,
    props_json TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create nodes table
CREATE TABLE IF NOT EXISTS nodes (
    object_id INTEGER PRIMARY KEY,
    props_json TEXT NOT NULL,
    volumes_json TEXT,
    mute BOOLEAN,
    peaks_json TEXT,
    rate INTEGER,
    positions_json TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create devices table
CREATE TABLE IF NOT EXISTS devices (
    object_id INTEGER PRIMARY KEY,
    props_json TEXT NOT NULL,
    profile_index INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create device_profiles table
CREATE TABLE IF NOT EXISTS device_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    profile_index INTEGER NOT NULL,
    description TEXT NOT NULL,
    available BOOLEAN NOT NULL,
    classes_json TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES devices(object_id) ON DELETE CASCADE,
    UNIQUE(device_id, profile_index)
);

-- Create device_routes table
CREATE TABLE IF NOT EXISTS device_routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    route_index INTEGER NOT NULL,
    route_device INTEGER NOT NULL,
    profiles_json TEXT NOT NULL,
    description TEXT NOT NULL,
    available BOOLEAN NOT NULL,
    volumes_json TEXT NOT NULL,
    mute BOOLEAN NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES devices(object_id) ON DELETE CASCADE,
    UNIQUE(device_id, route_index)
);

-- Create device_enum_routes table
CREATE TABLE IF NOT EXISTS device_enum_routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    enum_route_index INTEGER NOT NULL,
    description TEXT NOT NULL,
    available BOOLEAN NOT NULL,
    profiles_json TEXT NOT NULL,
    devices_json TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES devices(object_id) ON DELETE CASCADE,
    UNIQUE(device_id, enum_route_index)
);

-- Create links table
CREATE TABLE IF NOT EXISTS links (
    object_id INTEGER PRIMARY KEY,
    output_id INTEGER NOT NULL,
    input_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create metadata table
CREATE TABLE IF NOT EXISTS metadata (
    object_id INTEGER PRIMARY KEY,
    metadata_name TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create metadata_properties table
CREATE TABLE IF NOT EXISTS metadata_properties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metadata_id INTEGER NOT NULL,
    subject INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (metadata_id) REFERENCES metadata(object_id) ON DELETE CASCADE,
    UNIQUE(metadata_id, subject, key)
);

-- Create indexes for faster lookups
CREATE INDEX IF NOT EXISTS idx_clients_object_id ON clients(object_id);
CREATE INDEX IF NOT EXISTS idx_nodes_object_id ON nodes(object_id);
CREATE INDEX IF NOT EXISTS idx_devices_object_id ON devices(object_id);
CREATE INDEX IF NOT EXISTS idx_links_object_id ON links(object_id);
CREATE INDEX IF NOT EXISTS idx_metadata_object_id ON metadata(object_id);
CREATE INDEX IF NOT EXISTS idx_metadata_name ON metadata(metadata_name);
CREATE INDEX IF NOT EXISTS idx_links_output_id ON links(output_id);
CREATE INDEX IF NOT EXISTS idx_links_input_id ON links(input_id);
CREATE INDEX IF NOT EXISTS idx_device_profiles_device_id ON device_profiles(device_id);
CREATE INDEX IF NOT EXISTS idx_device_routes_device_id ON device_routes(device_id);
CREATE INDEX IF NOT EXISTS idx_device_enum_routes_device_id ON device_enum_routes(device_id);
CREATE INDEX IF NOT EXISTS idx_metadata_properties_metadata_id ON metadata_properties(metadata_id);