use scylla::client::session::Session;
use scylla::response::query_result::QueryResult;
use tracing::info;

/// Initialize database schema for Sentinel
pub async fn init_schema(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    info!("Initializing Sentinel database schema...");

    // Create keyspace
    let create_keyspace = "
        CREATE KEYSPACE IF NOT EXISTS sentinel
        WITH REPLICATION = {
            'class': 'SimpleStrategy',
            'replication_factor': 1
        }
    ";

    session.query_unpaged(create_keyspace, &[]).await?;
    info!("Keyspace 'sentinel' created successfully");

    // Use the keyspace
    session.query_unpaged("USE sentinel", &[]).await?;

    // Create relation_tuples table for storing Zanzibar tuples
    let create_tuples_table = "
        CREATE TABLE IF NOT EXISTS relation_tuples (
            namespace text,
            object_id text,
            relation text,
            user_type text,
            user_id text,
            created_at timestamp,
            PRIMARY KEY ((namespace, object_id), relation, user_type, user_id)
        )
    ";

    session.query_unpaged(create_tuples_table, &[]).await?;
    info!("Table 'relation_tuples' created successfully");

    // Create namespaces table for storing namespace configurations
    let create_namespaces_table = "
        CREATE TABLE IF NOT EXISTS namespaces (
            name text PRIMARY KEY,
            config text,
            created_at timestamp,
            updated_at timestamp
        )
    ";

    session.query_unpaged(create_namespaces_table, &[]).await?;
    info!("Table 'namespaces' created successfully");

    // Create changelog table for tracking changes
    let create_changelog_table = "
        CREATE TABLE IF NOT EXISTS changelog (
            id uuid,
            namespace text,
            object_id text,
            relation text,
            user_type text,
            user_id text,
            operation text,
            timestamp timestamp,
            PRIMARY KEY (id, timestamp)
        ) WITH CLUSTERING ORDER BY (timestamp DESC)
    ";

    session.query_unpaged(create_changelog_table, &[]).await?;
    info!("Table 'changelog' created successfully");

    info!("Database schema initialization completed");
    Ok(())
}

/// Test database connection
pub async fn test_connection(session: &Session) -> Result<QueryResult, Box<dyn std::error::Error>> {
    info!("Testing ScyllaDB connection...");
    
    let result = session.query_unpaged("SELECT release_version FROM system.local", &[]).await?;
    info!("ScyllaDB connection test successful");
    
    Ok(result)
}