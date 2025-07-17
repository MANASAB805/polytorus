-- PostgreSQL initialization script for Polytorus smart contract storage
-- This script sets up the database schema and initial configuration

-- Create the smart_contracts schema
CREATE SCHEMA IF NOT EXISTS smart_contracts;

-- Set the search path to include our schema
ALTER DATABASE polytorus_test SET search_path TO smart_contracts, public;

-- Grant permissions to the polytorus_test user
GRANT ALL PRIVILEGES ON SCHEMA smart_contracts TO polytorus_test;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA smart_contracts TO polytorus_test;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA smart_contracts TO polytorus_test;

-- Create extension for UUID generation if needed
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create a function to update the updated_at timestamp
CREATE OR REPLACE FUNCTION smart_contracts.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Note: The actual tables will be created by the Rust application
-- when it initializes the database schema. This script just sets up
-- the basic database structure and permissions.

-- Create a test user for additional testing scenarios
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'polytorus_readonly') THEN
        CREATE ROLE polytorus_readonly;
    END IF;
END
$$;

GRANT CONNECT ON DATABASE polytorus_test TO polytorus_readonly;
GRANT USAGE ON SCHEMA smart_contracts TO polytorus_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA smart_contracts TO polytorus_readonly;

-- Log the initialization (pg_stat_statements extension not available in this setup)

-- Create a simple logging table for test purposes
CREATE TABLE IF NOT EXISTS smart_contracts.test_log (
    id SERIAL PRIMARY KEY,
    message TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

INSERT INTO smart_contracts.test_log (message) VALUES ('Database initialized successfully');

-- Display initialization status
SELECT 'PostgreSQL database initialized for Polytorus testing' AS status;
