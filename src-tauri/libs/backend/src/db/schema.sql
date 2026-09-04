CREATE TABLE stations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    station_type TEXT NOT NULL CHECK (station_type IN ('master', 'slave')),
    description TEXT,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE master_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    protocol_type TEXT NOT NULL CHECK (protocol_type = 'iec104'),
    connection_params TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    auto_connect INTEGER NOT NULL DEFAULT 0,
    auto_start_data_transfer INTEGER NOT NULL DEFAULT 1,
    auto_gi INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 3,
    retry_interval INTEGER NOT NULL DEFAULT 5,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (station_id) REFERENCES stations(id) ON DELETE CASCADE
);

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    description TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE slave_station_profiles (
    station_id INTEGER PRIMARY KEY,
    link_params TEXT,
    initialization_profile TEXT,
    simulation_profile TEXT,
    redundancy_mode TEXT,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (station_id) REFERENCES stations(id) ON DELETE CASCADE
);

CREATE TABLE master_slaves (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    common_address INTEGER NOT NULL CHECK (common_address BETWEEN 0 AND 65535),
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (connection_id) REFERENCES master_links(id) ON DELETE CASCADE,
    UNIQUE (connection_id, common_address)
);

CREATE TABLE master_slave_asdu_aliases (
    slave_id INTEGER NOT NULL,
    type_name TEXT NOT NULL,
    alias TEXT NOT NULL,
    PRIMARY KEY (slave_id, type_name),
    FOREIGN KEY (slave_id) REFERENCES master_slaves(id) ON DELETE CASCADE
);

CREATE TABLE master_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_slave_id INTEGER NOT NULL,
    address INTEGER NOT NULL CHECK (address BETWEEN 1 AND 16777215),
    name TEXT NOT NULL,
    type_id INTEGER NOT NULL CHECK (type_id BETWEEN 1 AND 127),
    data_type TEXT NOT NULL,
    description TEXT,
    default_value TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (connection_slave_id) REFERENCES master_slaves(id) ON DELETE CASCADE,
    UNIQUE (connection_slave_id, address)
);

CREATE TABLE slave_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    common_address INTEGER NOT NULL CHECK (common_address BETWEEN 0 AND 65535),
    station_policy_override TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (station_id) REFERENCES stations(id) ON DELETE CASCADE,
    UNIQUE (station_id, common_address)
);

CREATE TABLE slave_device_asdu_aliases (
    slave_id INTEGER NOT NULL,
    type_name TEXT NOT NULL,
    alias TEXT NOT NULL,
    PRIMARY KEY (slave_id, type_name),
    FOREIGN KEY (slave_id) REFERENCES slave_devices(id) ON DELETE CASCADE
);

CREATE TABLE slave_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slave_station_id INTEGER NOT NULL,
    address INTEGER NOT NULL CHECK (address BETWEEN 1 AND 16777215),
    name TEXT NOT NULL,
    type_id INTEGER NOT NULL CHECK (type_id BETWEEN 1 AND 127),
    data_type TEXT NOT NULL,
    unit TEXT,
    description TEXT,
    min_value REAL,
    max_value REAL,
    default_value TEXT,
    value TEXT,
    control_ioa INTEGER,
    gi_group INTEGER CHECK (gi_group BETWEEN 1 AND 16),
    counter_group INTEGER CHECK (counter_group BETWEEN 1 AND 4),
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (slave_station_id) REFERENCES slave_devices(id) ON DELETE CASCADE,
    UNIQUE (slave_station_id, address)
);

CREATE INDEX idx_stations_type ON stations(station_type);
CREATE INDEX idx_master_links_station ON master_links(station_id);
CREATE INDEX idx_app_settings_category ON app_settings(category);
CREATE INDEX idx_master_slaves_connection ON master_slaves(connection_id);
CREATE INDEX idx_connection_slave_points_slave_type ON master_points(connection_slave_id, type_id);
CREATE INDEX idx_slave_entries_station ON slave_devices(station_id);
CREATE INDEX idx_slave_points_slave_type ON slave_points(slave_station_id, type_id);

PRAGMA user_version = 1;
